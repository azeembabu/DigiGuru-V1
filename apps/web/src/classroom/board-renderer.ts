/**
 * The whiteboard renderer: board ops -> Fabric.js canvas.
 *
 * **This file is where NN-1 is honoured on the client.** The gateway holds the
 * audio for turn N until we ACK the board for turn N. So the contract this
 * module owes the rest of the system is narrow and absolute:
 *
 *   `apply()` resolves only after the ops are actually on screen.
 *
 * Not after they are added to the canvas model — after the paint. Resolving
 * early would have us ACK a board the student cannot see yet, which converts
 * NN-1 from a guarantee into a lie that no metric would catch: `acked_ms` would
 * look healthy while audio genuinely ran ahead of the visual.
 *
 * Everything else here serves that: ops are applied to the canvas with rendering
 * suspended, then committed in a single `requestAnimationFrame`, so a burst of
 * ops is one paint (`.claude/rules/whiteboard-sync.md` "Canvas", 60 FPS target
 * on low-spec mobile).
 *
 * Malayalam: `whiteboard-sync.md` requires a subset Noto Sans Malayalam and
 * conjunct verification. The font is declared in CSS; this module only names it,
 * and `fontFamily` falls back through the system stack so a failed font load
 * degrades to tofu-free text rather than to nothing.
 */

import * as fabric from "fabric";

import type { BoardOp } from "./protocol";

/**
 * The lowest the flow cursor may reach before the board pages. Well short of
 * 1.0 because the transcript overlay occupies the bottom of the slate.
 */
const FLOW_BOTTOM = 0.72;

/** Matches the `HOLD_MAX` the gateway enforces, for the renderer's own warning. */
export const HOLD_MAX_MS = 400;

const BOARD_FONT =
  '"Noto Sans Malayalam", "Noto Sans", system-ui, -apple-system, "Segoe UI", sans-serif';

/**
 * The chalkboard palette.
 *
 * Chalk is never pure white and never fully opaque — a solid `#fff` on green
 * reads as a printed slide, not as something a teacher wrote. Each colour here
 * is an off-white or a pale pastel at partial opacity, which is what makes the
 * green board texture show faintly through the strokes.
 */
const BOARD_GREEN = "#35674f";
const CHALK_WHITE = "#f3f1e7";
const CHALK_BODY = "rgba(238, 236, 224, 0.86)";
const CHALK_MINT = "rgba(196, 235, 205, 0.9)";
const CHALK_AMBER = "rgba(244, 228, 160, 0.9)";

/** Shared look for the `draw` op's shape primitives — chalk lines, not vector art. */
const SKETCH_STROKE = CHALK_AMBER;
/**
 * A soft same-colour glow rather than a drop shadow: chalk scatters dust around
 * a stroke, it does not cast a shadow. An offset shadow made every shape look
 * like a floating UI card, which is the opposite of the blackboard reading.
 */
const SKETCH_SHADOW = new fabric.Shadow({
  color: "rgba(244, 228, 160, 0.45)",
  blur: 7,
  offsetX: 0,
  offsetY: 0,
});

/**
 * Chalk text gets the same treatment — a faint halo in its own colour, which is
 * what stops crisp anti-aliased glyphs from reading as a projected slide.
 */
function chalkGlow(color: string): fabric.Shadow {
  return new fabric.Shadow({ color, blur: 5, offsetX: 0, offsetY: 0 });
}

/**
 * The green slate itself: board colour with chalk dust smeared over it, painted
 * at the exact size of the canvas and used as a **non-repeating** background.
 *
 * It was a repeating tile first, and that was wrong twice over. Even with every
 * mark drawn nine times so it wrapped across the tile edges, the repeat was
 * plainly visible as vertical bands: the eye picks out a recurring arrangement
 * of smudges long before it picks out a seam, so making the seam invisible does
 * not make the tile invisible. Painting once at canvas size has no period at
 * all, and costs one fill of a few hundred KB at mount and on resize — nothing,
 * against a surface that then never repaints.
 *
 * The marks are seeded deterministically rather than from `Math.random()`, so a
 * resize, a remount and a replay of the same op-log all produce the same board.
 * A texture that reshuffles on every re-render is visible as a flicker.
 */
function makeChalkboardTexture(width: number, height: number): fabric.Pattern {
  const slate = document.createElement("canvas");
  slate.width = Math.max(1, Math.round(width));
  slate.height = Math.max(1, Math.round(height));
  const ctx = slate.getContext("2d");
  if (ctx) {
    ctx.fillStyle = BOARD_GREEN;
    ctx.fillRect(0, 0, slate.width, slate.height);

    // A cheap deterministic LCG — see the doc comment on why this must not be
    // `Math.random()`.
    let seed = 0x2f6b4f;
    const next = () => {
      seed = (seed * 1664525 + 1013904223) >>> 0;
      return seed / 0x100000000;
    };

    // Scaled with the board so a wide canvas is not sparser than a narrow one.
    const density = Math.round((slate.width * slate.height) / 9000);

    ctx.lineCap = "round";
    for (let i = 0; i < density; i += 1) {
      const x = next() * slate.width;
      const y = next() * slate.height;
      const length = 60 + next() * 220;
      const angle = (next() - 0.5) * 0.7;
      ctx.strokeStyle = `rgba(255, 255, 255, ${0.008 + next() * 0.014})`;
      ctx.lineWidth = 12 + next() * 34;
      ctx.beginPath();
      ctx.moveTo(x, y);
      ctx.lineTo(x + Math.cos(angle) * length, y + Math.sin(angle) * length);
      ctx.stroke();
    }
    // A few darker passes so the dust reads as smeared rather than only added.
    for (let i = 0; i < Math.round(density / 5); i += 1) {
      const x = next() * slate.width;
      const y = next() * slate.height;
      ctx.fillStyle = `rgba(18, 46, 34, ${0.012 + next() * 0.018})`;
      ctx.beginPath();
      ctx.ellipse(
        x,
        y,
        60 + next() * 130,
        30 + next() * 70,
        next() * Math.PI,
        0,
        Math.PI * 2,
      );
      ctx.fill();
    }
  }
  return new fabric.Pattern({ source: slate, repeat: "no-repeat" });
}

/** Board ops use normalised 0..1 coordinates; the canvas has pixels. */
interface Viewport {
  width: number;
  height: number;
}

export interface BoardRendererOptions {
  /** Called when an op cannot be rendered, so the client can send `board_error`. */
  onOpError?: (op: BoardOp, error: Error) => void;
}

/**
 * Wraps one Fabric canvas.
 *
 * Owns no React state and no socket. The caller decides what to do with the
 * promise `apply()` returns — which is what lets the ACK timing be tested
 * without a browser tab.
 */
export class BoardRenderer {
  private canvas: fabric.Canvas;
  /** Element ids -> objects, so `highlight` can target an earlier element. */
  private readonly byId = new Map<string, fabric.FabricObject>();
  /** Vertical cursor in normalised units, so successive ops stack down the board. */
  private flowY = 0.06;
  private disposed = false;

  constructor(
    element: HTMLCanvasElement,
    private readonly options: BoardRendererOptions = {},
  ) {
    this.canvas = new fabric.Canvas(element, {
      backgroundColor: makeChalkboardTexture(element.width, element.height),
      selection: false,
      renderOnAddRemove: false,
      // The board is a display surface, not a drawing tool: nothing the student
      // clicks should move or select an element.
      interactive: false,
      enableRetinaScaling: true,
    });
  }

  get viewport(): Viewport {
    return { width: this.canvas.getWidth(), height: this.canvas.getHeight() };
  }

  /** Resizes without clearing: a layout change must not wipe the lesson. */
  resize(width: number, height: number): void {
    if (this.disposed) return;
    this.canvas.setDimensions({ width, height });
    // The slate is painted at canvas size, so it has to be repainted when that
    // size changes or it would letterbox instead of filling the new board.
    this.canvas.backgroundColor = makeChalkboardTexture(width, height);
    this.canvas.requestRenderAll();
  }

  /**
   * Applies one turn's ops and resolves **after they have been painted**.
   *
   * The sequence is: suspend rendering, mutate, then a single rAF commit, then
   * resolve on the *following* frame callback. Resolving inside the same
   * callback that calls `renderAll()` would resolve before the browser has
   * composited, which is the subtle version of the early-ACK bug.
   */
  async apply(ops: BoardOp[], clearFirst: boolean): Promise<void> {
    if (this.disposed) return;

    if (clearFirst) this.clear();

    for (const op of ops) {
      try {
        this.applyOp(op);
      } catch (error) {
        // One bad op must not abandon the rest of the turn — a board missing a
        // figure is still a board, and the lesson continues.
        const err = error instanceof Error ? error : new Error(String(error));
        this.options.onOpError?.(op, err);
      }
    }

    await this.commit();
  }

  /** One paint for the whole batch, resolving once it is on screen. */
  private commit(): Promise<void> {
    return new Promise<void>((resolve) => {
      if (typeof requestAnimationFrame !== "function") {
        // Non-browser (SSR, tests): render synchronously and resolve. There is
        // no compositor to wait for.
        this.canvas.renderAll();
        resolve();
        return;
      }
      requestAnimationFrame(() => {
        if (this.disposed) {
          resolve();
          return;
        }
        this.canvas.renderAll();
        // Second frame: by the time this fires, the frame containing our paint
        // has been handed to the compositor.
        requestAnimationFrame(() => resolve());
      });
    });
  }

  clear(): void {
    this.canvas.remove(...this.canvas.getObjects());
    this.byId.clear();
    this.flowY = 0.06;
  }

  /**
   * Starts a fresh board page when the current one has no room left.
   *
   * The board flows downward and the canvas does not scroll, so without this an
   * op placed past the bottom edge is drawn where nobody can see it — the tutor
   * then narrates a visual the student does not have, which is the NN-1 failure
   * mode arriving by a different route.
   *
   * Clearing is the right response rather than shrinking or scrolling: the board
   * is already defined as a sequence of pages the server resets with
   * `clear_first` on block, chapter and session boundaries
   * (`.claude/rules/whiteboard-sync.md`), and a page break mid-topic reads
   * naturally for the same reason a real whiteboard gets wiped. The op-log is
   * unaffected — `board_events` keeps every op, so replay still reconstructs the
   * whole lesson.
   */
  private ensureRoom(neededNormalised: number): void {
    // Stops above the caption band, not at the very bottom edge: the transcript
    // overlays the lower ~32% of the slate, and chalk written behind it is
    // chalk the student cannot read.
    if (this.flowY + neededNormalised <= FLOW_BOTTOM) return;
    this.clear();
  }

  /**
   * Advances the flow cursor past an object that has just been laid out.
   *
   * This must be measured, never estimated. The first version advanced by a
   * fixed normalised step per op kind, which is correct only while every
   * heading fits on one line — and a long one does not. "Environmental
   * Studies: A Multidisciplinary Approach" wrapped to two lines, the cursor
   * moved by one, and the next op was drawn straight through it. Fabric has
   * already performed the wrap by the time the object is added, so `height` is
   * the true rendered height and there is nothing to guess at.
   *
   * `gapNormalised` is the space left under the block, so successive ops
   * breathe instead of stacking flush.
   */
  private advanceFlow(object: fabric.FabricObject, gapNormalised: number): void {
    const { height } = this.viewport;
    if (height <= 0) return;
    this.flowY += object.height / height + gapNormalised;
  }

  /**
   * Every object created below sets `originX: "left"` and `originY: "top"`
   * explicitly. **Do not remove these.**
   *
   * Fabric v6+ defaults both origins to `CENTER`, so `left`/`top` are read as
   * the object's CENTRE unless told otherwise. This layout code positions
   * everything by its top-left corner, so the defaults silently shifted every
   * element up and to the left by half its own size — a 624px-wide heading at
   * `left: 42.6` actually spanned −270..355, putting the first ~270px off the
   * canvas entirely. On screen that looked like the board was "clipped": the
   * head of every line was missing and only its tail was visible.
   *
   * It is invisible in the object model — `o.left` still reads 42.6, the
   * viewport transform is identity, and the canvas maps 1:1 to the screen — so
   * it can only be caught by rendering a known marker and comparing.
   */
  private applyOp(op: BoardOp): void {
    const { width, height } = this.viewport;

    switch (op.kind) {
      case "heading": {
        this.ensureRoom(0.1);
        const text = new fabric.Textbox(op.text, {
          left: width * 0.06,
          top: height * this.flowY,
          width: width * 0.88,
          fontSize: Math.max(20, Math.round(width * 0.038)),
          fontWeight: "700",
          fill: CHALK_WHITE,
          shadow: chalkGlow("rgba(243, 241, 231, 0.5)"),
          charSpacing: 24,
          fontFamily: BOARD_FONT,
          originX: "left",
          originY: "top",
          selectable: false,
          objectCaching: false,
        });
        this.add(op.id, text);
        this.advanceFlow(text, 0.025);
        break;
      }

      case "bullets": {
        this.ensureRoom(0.055 * Math.max(1, op.items.length) + 0.02);
        // One Textbox for the whole list rather than one per item: Fabric lays
        // out text on add, so N objects is N layout passes inside the hold
        // window for no visual benefit.
        const body = op.items.map((item) => `—  ${item}`).join("\n");
        const text = new fabric.Textbox(body, {
          left: width * 0.08,
          top: height * this.flowY,
          width: width * 0.84,
          fontSize: Math.max(15, Math.round(width * 0.024)),
          lineHeight: 1.6,
          fill: CHALK_BODY,
          shadow: chalkGlow("rgba(238, 236, 224, 0.35)"),
          charSpacing: 12,
          fontFamily: BOARD_FONT,
          originX: "left",
          originY: "top",
          selectable: false,
          objectCaching: false,
        });
        this.add(op.id, text);
        this.advanceFlow(text, 0.035);
        break;
      }

      case "math": {
        this.ensureRoom(0.08);
        // LaTeX is shown as monospace source, not typeset. A typesetting
        // dependency (KaTeX/MathJax) inside the 400 ms hold is a real risk, and
        // rendering nothing would be worse than rendering the source — the
        // tutor is also saying it aloud.
        const text = new fabric.Textbox(op.latex, {
          left: width * 0.08,
          top: height * this.flowY,
          width: width * 0.84,
          fontSize: Math.max(15, Math.round(width * 0.026)),
          fill: CHALK_MINT,
          shadow: chalkGlow("rgba(196, 235, 205, 0.4)"),
          fontFamily: '"Cascadia Code", "Consolas", ui-monospace, monospace',
          originX: "left",
          originY: "top",
          selectable: false,
          objectCaching: false,
        });
        this.add(op.id, text);
        this.advanceFlow(text, 0.03);
        break;
      }

      case "draw": {
        const pts = op.points.map((p) => ({ x: p.x * width, y: p.y * height }));
        if (pts.length < 2) throw new Error(`draw op ${op.id} needs at least two points`);

        if (op.shape === "rect") {
          const [a, b] = pts;
          this.add(
            op.id,
            new fabric.Rect({
              left: Math.min(a.x, b.x),
              top: Math.min(a.y, b.y),
              width: Math.abs(b.x - a.x),
              height: Math.abs(b.y - a.y),
              rx: 10,
              ry: 10,
              fill: "transparent",
              stroke: SKETCH_STROKE,
              strokeWidth: 2.5,
              strokeLineJoin: "round",
              shadow: SKETCH_SHADOW,
              originX: "left",
              originY: "top",
              selectable: false,
              objectCaching: false,
            }),
          );
        } else if (op.shape === "circle") {
          const [a, b] = pts;
          const radius = Math.hypot(b.x - a.x, b.y - a.y);
          this.add(
            op.id,
            new fabric.Circle({
              left: a.x - radius,
              top: a.y - radius,
              radius,
              fill: "transparent",
              stroke: SKETCH_STROKE,
              strokeWidth: 2.5,
              shadow: SKETCH_SHADOW,
              originX: "left",
              originY: "top",
              selectable: false,
              objectCaching: false,
            }),
          );
        } else {
          // Any other shape renders as a polyline through its points, which
          // covers line/arrow/freehand without inventing semantics the op
          // schema does not define.
          this.add(
            op.id,
            new fabric.Polyline(pts, {
              fill: "transparent",
              stroke: SKETCH_STROKE,
              strokeWidth: 2.5,
              strokeLineJoin: "round",
              strokeLineCap: "round",
              shadow: SKETCH_SHADOW,
              originX: "left",
              originY: "top",
              selectable: false,
              objectCaching: false,
            }),
          );
        }
        break;
      }

      case "image": {
        this.ensureRoom(0.2);
        // `reference` is a `doc:<uuid>#p<NN>-fig<M>` locator, not a URL, and
        // there is no figure-extraction endpoint yet. A labelled placeholder is
        // honest; a broken <img> is not, and silently skipping would leave the
        // tutor describing something absent.
        const group = new fabric.Rect({
          left: width * 0.08,
          top: height * this.flowY,
          width: width * 0.84,
          height: height * 0.16,
          fill: "rgba(255, 255, 255, 0.04)",
          stroke: "rgba(238, 236, 224, 0.45)",
          strokeDashArray: [6, 4],
          strokeWidth: 1,
          originX: "left",
          originY: "top",
          selectable: false,
          objectCaching: false,
        });
        this.add(op.id, group);
        const label = new fabric.Textbox(`figure ${op.reference}`, {
          left: width * 0.1,
          top: height * (this.flowY + 0.06),
          width: width * 0.8,
          fontSize: Math.max(12, Math.round(width * 0.018)),
          fill: "rgba(238, 236, 224, 0.6)",
          fontFamily: BOARD_FONT,
          originX: "left",
          originY: "top",
          selectable: false,
          objectCaching: false,
        });
        this.canvas.add(label);
        this.flowY += 0.2;
        break;
      }

      case "highlight": {
        const target = this.byId.get(op.target);
        if (!target) {
          // Per the op schema a highlight targets "a previously emitted
          // element". A missing target is a model or ordering bug, and it is
          // reported rather than silently ignored.
          throw new Error(`highlight target ${op.target} was never emitted`);
        }
        target.set({ backgroundColor: "rgba(244, 228, 160, 0.16)" });
        target.setCoords();
        break;
      }
    }
  }

  private add(id: string, object: fabric.FabricObject): void {
    this.canvas.add(object);
    this.byId.set(id, object);
  }

  /** Serialises the board as a JSON op-log snapshot — never an image. */
  toJSON(): unknown {
    return this.canvas.toJSON();
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.byId.clear();
    void this.canvas.dispose();
  }
}

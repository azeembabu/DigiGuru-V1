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

import { latexToSvg } from "./math-render";
import type { BoardOp } from "./protocol";

/**
 * The lowest the flow cursor may reach before the board pages. Well short of
 * 1.0 because the transcript overlay occupies the bottom of the slate.
 */
const FLOW_BOTTOM = 0.72;

/** Matches the `HOLD_MAX` the gateway enforces, for the renderer's own warning. */
export const HOLD_MAX_MS = 400;

/**
 * The board face — see `app/layout.tsx` for why this is Noto rather than a
 * handwriting font.
 *
 * Named literally rather than read from the `--font-board` CSS variable,
 * because Fabric measures text against a canvas 2D context and `ctx.font` does
 * not resolve `var()`. The fallbacks matter: if the webfont has not arrived,
 * the next entries still shape Malayalam rather than dropping to a face that
 * renders conjuncts as boxes.
 */
const BOARD_FONT =
  '"Noto Sans Malayalam", "Noto Sans", "Nirmala UI", system-ui, -apple-system, sans-serif';

/**
 * Monospace for formulas and code, chosen for symbol coverage rather than
 * looks: a maths line that falls back to a face without Greek or operators
 * prints boxes where the meaning is.
 */
const MONO_FONT = '"Cascadia Code", "Consolas", "DejaVu Sans Mono", ui-monospace, monospace';

/**
 * Resolves once the board face is actually loaded.
 *
 * Fabric lays a Textbox out the moment it is constructed, so text built before
 * the webfont arrives is measured against the fallback and keeps those wrong
 * line breaks after the swap — the wrap is baked into the object, not
 * recomputed on repaint. Waiting costs nothing after the first turn (the
 * promise is already settled) and the NN-1 hold has 400 ms of headroom.
 */
function boardFontsReady(): Promise<unknown> {
  if (typeof document === "undefined" || !("fonts" in document)) return Promise.resolve();
  return document.fonts.ready;
}

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

/**
 * Chalk colours for chart series, in order.
 *
 * Chosen to stay distinguishable on green *and* in greyscale — a board gets
 * printed and photographed, and a palette that relies on hue alone becomes one
 * grey blob. These vary in lightness as well as hue.
 */
const CHART_COLOURS = [
  "rgba(244, 228, 160, 0.85)",
  "rgba(196, 235, 205, 0.85)",
  "rgba(186, 214, 240, 0.85)",
  "rgba(240, 200, 190, 0.85)",
  "rgba(224, 210, 240, 0.85)",
  "rgba(250, 246, 220, 0.85)",
  "rgba(170, 205, 190, 0.85)",
  "rgba(214, 224, 200, 0.85)",
];

/**
 * A filled wedge, as a Fabric path.
 *
 * `fabric.Circle` has `startAngle`/`endAngle` but strokes an arc rather than
 * filling a wedge to the centre, so a pie built from circles is a set of
 * crescents. The path is the shape actually wanted: centre, line out, arc
 * round, close.
 */
function pieSlice(
  cx: number,
  cy: number,
  radius: number,
  start: number,
  end: number,
  fill: string,
): fabric.Path {
  const x1 = cx + radius * Math.cos(start);
  const y1 = cy + radius * Math.sin(start);
  const x2 = cx + radius * Math.cos(end);
  const y2 = cy + radius * Math.sin(end);
  const largeArc = end - start > Math.PI ? 1 : 0;
  // A full circle cannot be drawn as a single arc — start and end coincide and
  // the path collapses — so it is drawn as two half circles.
  const d =
    end - start >= Math.PI * 2 - 1e-6
      ? `M ${cx - radius} ${cy} A ${radius} ${radius} 0 1 1 ${cx + radius} ${cy} A ${radius} ${radius} 0 1 1 ${cx - radius} ${cy} Z`
      : `M ${cx} ${cy} L ${x1} ${y1} A ${radius} ${radius} 0 ${largeArc} 1 ${x2} ${y2} Z`;

  return new fabric.Path(d, {
    fill,
    stroke: "rgba(20, 48, 36, 0.35)",
    strokeWidth: 1,
    originX: "left",
    originY: "top",
    selectable: false,
    objectCaching: false,
  });
}

/** Whole numbers stay whole; fractions keep one decimal and no trailing zero. */
function formatValue(value: number): string {
  return Number.isInteger(value) ? String(value) : value.toFixed(1).replace(/\.0$/, "");
}

/** Board ops use normalised 0..1 coordinates; the canvas has pixels. */
interface Viewport {
  width: number;
  height: number;
}

export interface BoardRendererOptions {
  /** Called when an op cannot be rendered, so the client can send `board_error`. */
  onOpError?: (op: BoardOp, error: Error) => void;
  /** Called whenever the page count or the viewed page changes. */
  onPageChange?: (viewing: number, total: number) => void;
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
  /**
   * Signatures of the content already written on the current page.
   *
   * The tutor re-emits a heading and its bullets when it returns to a topic —
   * observed live as the same "പരിസ്ഥിതി പഠനം (Environmental Studies)" block
   * written twice, one under the other, which reads as a stutter rather than
   * as teaching. The system instruction asks it not to, but a prompt is a
   * request; this is the mechanical half, and it is cheap: content already
   * visible on this page is not written again.
   *
   * Scoped to the page, not to the session: after a `clear_first` or a page
   * break the board is empty, and re-stating the heading of the topic being
   * continued is then correct rather than repetitive.
   */
  private readonly onPage = new Set<string>();
  /** Vertical cursor in normalised units, so successive ops stack down the board. */
  private flowY = 0.06;
  /**
   * The unit being taught, pinned along the top of the slate.
   *
   * Kept out of the op flow and re-drawn after every clear, because it is not
   * part of any turn: the board pages itself as it fills, and a heading that
   * scrolled away with the first page would leave the student looking at a
   * slate with no idea which unit is on it.
   */
  private unitLabel: string | null = null;
  private unitLabelObject: fabric.FabricObject | null = null;
  /**
   * Completed pages, oldest first, each holding the objects drawn on it.
   *
   * The board used to *wipe* itself when it filled up, so the first half of a
   * lesson was simply gone — a student who looked away during the explanation
   * had no way back to it. Keeping the objects (rather than a JSON snapshot)
   * means turning a page is an add/remove on the canvas with nothing to
   * re-parse, re-measure or re-lay-out.
   */
  private readonly pages: fabric.FabricObject[][] = [];
  /** Objects on the page currently being written to. */
  private live: fabric.FabricObject[] = [];
  /** Which page the student is looking at; equals `pages.length` when live. */
  private viewing = 0;
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

  /**
   * Names the unit along the top of the board.
   *
   * Idempotent, so the caller can set it on every `session_ready` without
   * having to track whether it has changed.
   */
  setUnitLabel(label: string | null): void {
    if (this.disposed || this.unitLabel === label) return;
    this.unitLabel = label;
    this.drawUnitLabel();
    this.canvas.requestRenderAll();
  }

  /** (Re)draws the pinned unit heading and the rule under it. */
  private drawUnitLabel(): void {
    if (this.unitLabelObject) {
      this.canvas.remove(this.unitLabelObject);
      this.unitLabelObject = null;
    }
    if (this.unitLabel === null || this.unitLabel.trim() === "") return;

    const { width, height } = this.viewport;
    const fontSize = Math.max(12, Math.round(width * 0.0155));
    const text = new fabric.Textbox(this.unitLabel, {
      left: width * 0.06,
      top: height * 0.022,
      width: width * 0.88,
      fontSize,
      fontWeight: "600",
      fill: "rgba(244, 228, 160, 0.92)",
      fontFamily: BOARD_FONT,
      originX: "left",
      originY: "top",
      selectable: false,
      objectCaching: false,
    });
    const rule = new fabric.Line(
      [width * 0.06, height * 0.022 + fontSize * 1.6, width * 0.94, height * 0.022 + fontSize * 1.6],
      {
        stroke: "rgba(238, 236, 224, 0.25)",
        strokeWidth: 1,
        selectable: false,
        objectCaching: false,
      },
    );

    const group = new fabric.Group([text, rule], {
      selectable: false,
      objectCaching: false,
    });
    this.canvas.add(group);
    // Behind the lesson: an op that happens to overlap should cover the
    // heading rather than be covered by it.
    this.canvas.sendObjectToBack(group);
    this.unitLabelObject = group;
  }

  get viewport(): Viewport {
    return { width: this.canvas.getWidth(), height: this.canvas.getHeight() };
  }

  /** Resizes without clearing: a layout change must not wipe the lesson. */
  resize(width: number, height: number): void {
    if (this.disposed) return;
    this.canvas.setDimensions({ width, height });
    // Positioned in pixels, so it has to be laid out again at the new size.
    this.unitLabelObject = null;
    this.drawUnitLabel();
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

    // Before any text is constructed — see `boardFontsReady`.
    await boardFontsReady();
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

  /**
   * Wipes the board completely — every page.
   *
   * This is `clear_first`, which the server sends on a block, chapter or
   * session boundary (`whiteboard-sync.md`). It is a harder thing than the page
   * break `ensureRoom` performs: there is no turning back to a topic that has
   * been left behind, so the history goes with it.
   */
  clear(): void {
    this.canvas.remove(...this.canvas.getObjects());
    this.byId.clear();
    this.onPage.clear();
    this.pages.length = 0;
    this.live = [];
    this.viewing = 0;
    this.flowY = this.startFlow();
    // The heading belongs to the session, not to the page that was just wiped.
    this.unitLabelObject = null;
    this.drawUnitLabel();
    this.notifyPages();
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
    this.turnPage();
  }

  /**
   * Files the current page and starts a fresh one.
   *
   * The filed page keeps its objects, so `goToPage` can put it back exactly as
   * it was. Only the *canvas* is cleared; `byId` keeps every element, because a
   * `highlight` may still target something written two pages ago and the
   * student can turn back to see it.
   */
  private turnPage(): void {
    if (this.live.length > 0) {
      this.pages.push(this.live);
      this.live = [];
    }
    this.canvas.remove(...this.pageObjects());
    this.onPage.clear();
    this.flowY = this.startFlow();
    this.viewing = this.pages.length;
    this.notifyPages();
  }

  /** Objects belonging to the lesson, i.e. everything but the pinned heading. */
  private pageObjects(): fabric.FabricObject[] {
    return this.canvas
      .getObjects()
      .filter((object) => object !== this.unitLabelObject);
  }

  private startFlow(): number {
    return this.unitLabel ? 0.11 : 0.06;
  }

  /** How many pages exist, counting the one being written. */
  get pageCount(): number {
    return this.pages.length + (this.live.length > 0 ? 1 : 0);
  }

  /** The page on screen, 0-based. */
  get pageIndex(): number {
    return this.viewing;
  }

  /**
   * Shows an earlier page, or returns to the live one.
   *
   * Turning back does not stop the lesson: the tutor keeps writing to the live
   * page, and the next op snaps the student forward again (`applyOp` calls
   * `returnToLive`). That is deliberate — NN-1 exists so the student is looking
   * at the thing being explained, and leaving them on page 1 while the tutor
   * narrates page 3 would break exactly that.
   */
  goToPage(index: number): void {
    if (this.disposed) return;
    const clamped = Math.max(0, Math.min(index, this.pageCount - 1));
    if (clamped === this.viewing) return;

    this.canvas.remove(...this.pageObjects());
    const target = clamped < this.pages.length ? this.pages[clamped] : this.live;
    for (const object of target) this.canvas.add(object);
    this.viewing = clamped;
    this.canvas.requestRenderAll();
    this.notifyPages();
  }

  /** Snaps back to the page being written, if the student had turned back. */
  private returnToLive(): void {
    if (this.viewing === this.pages.length) return;
    this.goToPage(this.pages.length);
  }

  private notifyPages(): void {
    this.options.onPageChange?.(this.viewing, Math.max(1, this.pageCount));
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
    // Whatever the student was reading, the tutor is about to explain this.
    this.returnToLive();
    const { width, height } = this.viewport;

    // Already on this page — see `onPage`. Silently skipped rather than
    // reported through `onOpError`: a repeat is not a render failure, and a
    // `board_error` would drop the turn to the text fallback for something
    // that rendered correctly the first time.
    const signature = contentSignature(op);
    if (signature !== null) {
      if (this.onPage.has(signature)) return;
      this.onPage.add(signature);
    }

    switch (op.kind) {
      case "heading": {
        this.ensureRoom(0.1);
        const text = new fabric.Textbox(op.text, {
          left: width * 0.06,
          top: height * this.flowY,
          width: width * 0.88,
          fontSize: Math.max(17, Math.round(width * 0.024)),
          fontWeight: "600",
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
          fontSize: Math.max(13, Math.round(width * 0.0165)),
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
        // Typeset properly — see `math-render.ts`. The source text below is the
        // fallback for LaTeX MathJax cannot parse; it is drawn immediately so
        // the board is never empty, and replaced in place once the SVG is
        // ready. A student copying from the board must see a fraction, not
        // `\frac{a}{b}`.
        void this.typesetMath(op.id, op.latex, width, height * this.flowY);
        const text = new fabric.Textbox(op.latex, {
          left: width * 0.08,
          top: height * this.flowY,
          width: width * 0.84,
          fontSize: Math.max(13, Math.round(width * 0.018)),
          fill: CHALK_MINT,
          shadow: chalkGlow("rgba(196, 235, 205, 0.4)"),
          fontFamily: MONO_FONT,
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
          fontSize: Math.max(11, Math.round(width * 0.013)),
          fill: "rgba(238, 236, 224, 0.6)",
          fontFamily: BOARD_FONT,
          originX: "left",
          originY: "top",
          selectable: false,
          objectCaching: false,
        });
        this.canvas.add(label);
        this.live.push(label);
        this.flowY += 0.2;
        break;
      }

      case "bar_chart": {
        this.ensureRoom(0.34);
        this.drawBarChart(op.id, op.title, op.series, width, height);
        break;
      }

      case "pie_chart": {
        this.ensureRoom(0.34);
        this.drawPieChart(op.id, op.title, op.series, width, height);
        break;
      }

      case "flow": {
        this.ensureRoom(0.2);
        this.drawFlow(op.id, op.title, op.steps, width, height);
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

  /**
   * A bar chart drawn to the widest bar, not to a round number.
   *
   * Scaling to the maximum value means the tallest bar always fills the plot,
   * so two bars that differ by 5% look 5% different rather than both looking
   * full. Every bar is labelled with its own value, because a board is read
   * from across a room and nobody reads an axis from there.
   */
  private drawBarChart(
    id: string,
    title: string,
    series: { label: string; value: number }[],
    width: number,
    height: number,
  ): void {
    const top = height * this.flowY;
    const plotHeight = height * 0.2;
    const left = width * 0.08;
    const plotWidth = width * 0.84;
    const label = this.chartTitle(title, left, top, width);

    const max = Math.max(...series.map((point) => point.value));
    const slot = plotWidth / series.length;
    const barWidth = Math.min(slot * 0.62, width * 0.09);
    const baseline = top + label.height + 8 + plotHeight;

    const parts: fabric.FabricObject[] = [label];
    series.forEach((point, index) => {
      // `max` cannot be 0 — the gateway rejects an all-zero series — so this
      // division is safe, and a genuine 0 renders as a hairline rather than
      // vanishing, which is the honest depiction of "none".
      const barHeight = Math.max(2, (point.value / max) * plotHeight);
      const centre = left + slot * index + slot / 2;

      parts.push(
        new fabric.Rect({
          left: centre - barWidth / 2,
          top: baseline - barHeight,
          width: barWidth,
          height: barHeight,
          fill: CHART_COLOURS[index % CHART_COLOURS.length],
          rx: 2,
          ry: 2,
          originX: "left",
          originY: "top",
          selectable: false,
          objectCaching: false,
        }),
        this.chartText(formatValue(point.value), centre, baseline - barHeight - 16, slot, "center"),
        this.chartText(point.label, centre, baseline + 6, slot, "center"),
      );
    });

    // The baseline itself, so bars sit on something rather than floating.
    parts.push(
      new fabric.Line([left, baseline, left + plotWidth, baseline], {
        stroke: "rgba(238, 236, 224, 0.45)",
        strokeWidth: 1,
        selectable: false,
        objectCaching: false,
      }),
    );

    this.addGroup(id, parts);
    this.flowY += (label.height + plotHeight + 46) / height + 0.02;
  }

  /**
   * A pie drawn from the real totals.
   *
   * Proportions are computed here from the raw values rather than taken from
   * the model, so a set of slices always sums to the whole circle — a chart
   * handed pre-computed percentages can be internally inconsistent and there is
   * no way to notice from looking at it.
   */
  private drawPieChart(
    id: string,
    title: string,
    series: { label: string; value: number }[],
    width: number,
    height: number,
  ): void {
    const top = height * this.flowY;
    const label = this.chartTitle(title, width * 0.08, top, width);
    const radius = Math.min(height * 0.1, width * 0.1);
    const cx = width * 0.22;
    const cy = top + label.height + 12 + radius;
    const total = series.reduce((sum, point) => sum + point.value, 0);

    const parts: fabric.FabricObject[] = [label];
    let start = -Math.PI / 2; // start at 12 o'clock, as a reader expects

    series.forEach((point, index) => {
      const sweep = total > 0 ? (point.value / total) * Math.PI * 2 : 0;
      if (sweep > 0) {
        parts.push(
          pieSlice(cx, cy, radius, start, start + sweep, CHART_COLOURS[index % CHART_COLOURS.length]),
        );
      }
      start += sweep;

      // A legend, not labels on the slices: a thin slice has no room for text,
      // and leader lines on a chalkboard read as noise.
      const legendY = top + label.height + 12 + index * 22;
      parts.push(
        new fabric.Rect({
          left: cx + radius + 24,
          top: legendY + 3,
          width: 10,
          height: 10,
          fill: CHART_COLOURS[index % CHART_COLOURS.length],
          originX: "left",
          originY: "top",
          selectable: false,
          objectCaching: false,
        }),
        this.chartText(
          `${point.label} — ${formatValue(point.value)}` +
            (total > 0 ? ` (${Math.round((point.value / total) * 100)}%)` : ""),
          cx + radius + 40,
          legendY,
          width * 0.5,
          "left",
        ),
      );
    });

    this.addGroup(id, parts);
    const legendHeight = series.length * 22;
    this.flowY += (label.height + 12 + Math.max(radius * 2, legendHeight) + 16) / height + 0.02;
  }

  /** Boxes joined by arrows, wrapping to a second row when the board runs out. */
  private drawFlow(
    id: string,
    title: string,
    steps: string[],
    width: number,
    height: number,
  ): void {
    const top = height * this.flowY;
    const label = this.chartTitle(title, width * 0.08, top, width);
    const left = width * 0.08;
    const usable = width * 0.84;
    const gap = Math.max(18, width * 0.018);
    const boxWidth = (usable - gap * (steps.length - 1)) / steps.length;
    const boxHeight = Math.max(42, height * 0.075);
    const boxTop = top + label.height + 12;

    const parts: fabric.FabricObject[] = [label];
    steps.forEach((step, index) => {
      const boxLeft = left + (boxWidth + gap) * index;
      parts.push(
        new fabric.Rect({
          left: boxLeft,
          top: boxTop,
          width: boxWidth,
          height: boxHeight,
          fill: "rgba(255, 255, 255, 0.05)",
          stroke: CHART_COLOURS[index % CHART_COLOURS.length],
          strokeWidth: 1.5,
          rx: 6,
          ry: 6,
          originX: "left",
          originY: "top",
          selectable: false,
          objectCaching: false,
        }),
        new fabric.Textbox(step, {
          left: boxLeft + 6,
          top: boxTop + 8,
          width: boxWidth - 12,
          fontSize: Math.max(11, Math.round(width * 0.0125)),
          textAlign: "center",
          fill: CHALK_BODY,
          fontFamily: BOARD_FONT,
          originX: "left",
          originY: "top",
          selectable: false,
          objectCaching: false,
        }),
      );

      if (index < steps.length - 1) {
        const arrowY = boxTop + boxHeight / 2;
        const arrowFrom = boxLeft + boxWidth + 3;
        const arrowTo = boxLeft + boxWidth + gap - 3;
        parts.push(
          new fabric.Line([arrowFrom, arrowY, arrowTo, arrowY], {
            stroke: "rgba(238, 236, 224, 0.6)",
            strokeWidth: 1.5,
            selectable: false,
            objectCaching: false,
          }),
          // The head, as two short strokes — a triangle would need a polygon
          // and reads heavier than a chalk arrow should.
          new fabric.Polyline(
            [
              { x: arrowTo - 5, y: arrowY - 4 },
              { x: arrowTo, y: arrowY },
              { x: arrowTo - 5, y: arrowY + 4 },
            ],
            {
              fill: "transparent",
              stroke: "rgba(238, 236, 224, 0.6)",
              strokeWidth: 1.5,
              selectable: false,
              objectCaching: false,
            },
          ),
        );
      }
    });

    this.addGroup(id, parts);
    this.flowY += (label.height + 12 + boxHeight + 18) / height + 0.02;
  }

  /**
   * Replaces a formula's placeholder source text with the typeset SVG.
   *
   * Deliberately *not* awaited by `apply()`. MathJax's first call has to build
   * its whole TeX pipeline, which can take longer than the 400 ms NN-1 hold —
   * and blocking the board ACK on that would hold the tutor's audio back behind
   * a typesetting library, which is exactly the sort of thing `whiteboard-sync.md`
   * says the hold must never become. So the source text is painted inside the
   * turn and the typeset version swaps in a frame or two later.
   *
   * The swap keeps the element's id and flow position, so a later `highlight`
   * still finds it and nothing below it moves.
   */
  private async typesetMath(
    id: string,
    latex: string,
    width: number,
    top: number,
  ): Promise<void> {
    const svg = await latexToSvg(latex);
    if (svg === null || this.disposed) return;

    const placeholder = this.byId.get(id);
    // The board may have been cleared or paged while MathJax was working; if
    // the placeholder is gone, so is the turn it belonged to.
    if (!placeholder || !this.canvas.getObjects().includes(placeholder)) return;

    try {
      const loaded = await fabric.loadSVGFromString(svg);
      if (this.disposed) return;
      const objects = loaded.objects.filter((object): object is fabric.FabricObject =>
        Boolean(object),
      );
      if (objects.length === 0) return;

      const group = fabric.util.groupSVGElements(objects, loaded.options);

      // MathJax sizes in ex units, so an expression can come back at any
      // scale. Fit it to a readable height, and cap the width so a long
      // derivation shrinks rather than running off the slate.
      const targetHeight = Math.max(22, width * 0.028);
      const scale = Math.min(
        targetHeight / Math.max(group.height ?? 1, 1),
        (width * 0.84) / Math.max(group.width ?? 1, 1),
      );

      group.set({
        left: width * 0.08,
        top,
        scaleX: scale,
        scaleY: scale,
        // Chalk white, overriding MathJax's black paths — the glyphs are
        // filled paths, so this is the only way to colour them.
        fill: CHALK_MINT,
        originX: "left",
        originY: "top",
        selectable: false,
        objectCaching: false,
      });
      // `groupSVGElements` returns a Group for a multi-glyph expression and a
      // bare object for a single one, so the children are recoloured only when
      // there are children to recolour.
      if (group instanceof fabric.Group) {
        group.getObjects().forEach((child) => child.set({ fill: CHALK_MINT }));
      }

      this.canvas.remove(placeholder);
      this.canvas.add(group);
      this.byId.set(id, group);
      // Swap it inside whichever page it belongs to, so turning back to that
      // page still shows the typeset formula rather than the placeholder.
      const inLive = this.live.indexOf(placeholder);
      if (inLive >= 0) {
        this.live[inLive] = group;
      } else {
        for (const page of this.pages) {
          const at = page.indexOf(placeholder);
          if (at >= 0) {
            page[at] = group;
            break;
          }
        }
      }
      this.canvas.requestRenderAll();
    } catch {
      // Keep the source text. It is readable, and it is what was there before
      // this method existed.
    }
  }

  /** The caption above a chart. */
  private chartTitle(text: string, left: number, top: number, width: number): fabric.Textbox {
    return new fabric.Textbox(text, {
      left,
      top,
      width: width * 0.84,
      fontSize: Math.max(12, Math.round(width * 0.015)),
      fontWeight: "600",
      fill: CHALK_WHITE,
      fontFamily: BOARD_FONT,
      originX: "left",
      originY: "top",
      selectable: false,
      objectCaching: false,
    });
  }

  /** A small label inside a chart. */
  private chartText(
    text: string,
    centreOrLeft: number,
    top: number,
    slot: number,
    align: "center" | "left",
  ): fabric.Textbox {
    return new fabric.Textbox(text, {
      left: align === "center" ? centreOrLeft - slot / 2 : centreOrLeft,
      top,
      width: slot,
      fontSize: Math.max(10, Math.round(slot * 0.13)),
      textAlign: align,
      fill: CHALK_BODY,
      fontFamily: BOARD_FONT,
      originX: "left",
      originY: "top",
      selectable: false,
      objectCaching: false,
    });
  }

  /**
   * Adds a chart as one grouped object.
   *
   * Grouped so `highlight` can target the whole figure, and so a chart is one
   * object to the canvas rather than thirty — which matters for the 400 ms
   * NN-1 hold, since Fabric lays out every object as it is added.
   */
  private addGroup(id: string, parts: fabric.FabricObject[]): void {
    const group = new fabric.Group(parts, {
      selectable: false,
      objectCaching: false,
    });
    this.add(id, group);
  }

  private add(id: string, object: fabric.FabricObject): void {
    this.canvas.add(object);
    this.byId.set(id, object);
    this.live.push(object);
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

/**
 * What an op *says*, independent of the id it was emitted under.
 *
 * Ids are per-turn, so they cannot detect a repeat; the content can. Returns
 * `null` for ops that are meaningless to compare — a `highlight` targets an
 * element rather than adding content, and two `draw` ops with the same points
 * are usually a deliberate overlay rather than a mistake.
 */
function contentSignature(op: BoardOp): string | null {
  switch (op.kind) {
    case "heading":
      return `heading:${op.text.trim()}`;
    case "bullets":
      return `bullets:${op.items.map((item) => item.trim()).join("\u0000")}`;
    case "math":
      return `math:${op.latex.trim()}`;
    case "image":
      return `image:${op.reference}`;
    default:
      return null;
  }
}

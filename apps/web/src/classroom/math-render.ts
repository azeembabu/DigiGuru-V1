/**
 * LaTeX -> SVG, for the whiteboard.
 *
 * # Why this exists
 *
 * `math` ops were drawn as their own LaTeX source in a monospace face, so a
 * student copying from the board wrote down `\frac{a}{b}` instead of a
 * fraction, and `H_2O` instead of H₂O. On a board whose whole job is to show
 * the student what to write in their notebook, that is not a cosmetic problem —
 * it is the board saying something the textbook does not.
 *
 * # Why MathJax's SVG output and not KaTeX
 *
 * The board is a Fabric canvas, and a canvas cannot draw HTML. KaTeX emits HTML
 * plus CSS plus webfonts, so putting it on a canvas means rasterising it through
 * an SVG `foreignObject` — which silently drops the maths fonts unless they are
 * inlined, and renders boxes where the symbols should be.
 *
 * MathJax's SVG output converts every glyph to a `<path>`. The result is
 * self-contained geometry with no font dependency at all, which Fabric can load
 * directly and which scales and prints cleanly. That is worth the heavier
 * dependency, and it is dynamically imported so it costs nothing outside the
 * classroom.
 *
 * # Failure is never silent and never fatal
 *
 * Bad LaTeX from the model returns `null`, and the caller falls back to drawing
 * the source text as it did before. A formula the student can at least read in
 * source form beats an empty space on the board, and neither is allowed to take
 * down the turn (`whiteboard-sync.md`: an invalid op is dropped, the lesson
 * continues).
 */

/** Lazily built once; MathJax's setup is expensive and entirely reusable. */
let converter: Promise<(latex: string) => string | null> | null = null;

async function buildConverter(): Promise<(latex: string) => string | null> {
  const [{ mathjax }, { TeX }, { SVG }, { liteAdaptor }, { RegisterHTMLHandler }, { AllPackages }] =
    await Promise.all([
      import("mathjax-full/js/mathjax.js"),
      import("mathjax-full/js/input/tex.js"),
      import("mathjax-full/js/output/svg.js"),
      import("mathjax-full/js/adaptors/liteAdaptor.js"),
      import("mathjax-full/js/handlers/html.js"),
      import("mathjax-full/js/input/tex/AllPackages.js"),
    ]);

  // The lite adaptor keeps MathJax off the real DOM: this runs during the NN-1
  // hold, and a pass that touched the live document would force layout on the
  // page the board is trying to paint.
  const adaptor = liteAdaptor();
  RegisterHTMLHandler(adaptor);

  const document = mathjax.document("", {
    InputJax: new TeX({ packages: AllPackages }),
    OutputJax: new SVG({ fontCache: "local" }),
  });

  return (latex: string): string | null => {
    try {
      const node = document.convert(latex, { display: true });
      const svg = adaptor.innerHTML(node);
      // MathJax wraps the SVG in a container; a conversion that produced no
      // `<svg` at all means the input was not maths.
      return svg.includes("<svg") ? svg : null;
    } catch {
      return null;
    }
  };
}

/**
 * Renders one LaTeX expression to a standalone SVG string, or `null` if it
 * cannot be parsed.
 *
 * `fontCache: "local"` keeps each expression's glyph definitions inside its own
 * SVG. The alternative, a shared global cache, produces SVGs that reference
 * `<use>` targets living in a different element — which is exactly what breaks
 * when Fabric loads them one at a time.
 */
export async function latexToSvg(latex: string): Promise<string | null> {
  if (latex.trim() === "") return null;
  converter ??= buildConverter();
  try {
    return (await converter)(latex);
  } catch {
    // A failed *setup* must not poison later turns: drop the cached promise so
    // the next formula retries rather than inheriting the failure forever.
    converter = null;
    return null;
  }
}

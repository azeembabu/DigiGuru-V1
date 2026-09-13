import { IconSparkle } from "@/components/icons";

// The hero visual. Rather than a stock photo (Digi Guru has no product
// screenshots yet, and no real photography to composite), this shows the
// actual mechanism the product is built on: a citation, a board that fills in
// before the tutor speaks, and a live focus timer — see
// .claude/rules/whiteboard-sync.md and IMPLEMENTATION_PLAN.md NN-1/NN-3.
export function SessionPreview() {
  return (
    <div className="relative mx-auto w-full max-w-md lg:mx-0">
      {/* Portrait-glow gradient, per DESIGN.md §1.2, sized down behind the card */}
      <div
        aria-hidden="true"
        className="absolute -inset-6 -z-10 rounded-[36px] opacity-70 blur-2xl"
        style={{
          background:
            "radial-gradient(120% 120% at 30% 20%, #6E74E8 0%, #3E3FA8 55%, #171A33 100%)",
        }}
      />

      <div className="rounded-lg border border-ink-800 bg-ink-900 p-6 shadow-2xl shadow-black/40">
        {/* Session status bar */}
        <div className="flex items-center justify-between gap-4 border-b border-ink-800 pb-4">
          <div className="flex items-center gap-2">
            <span className="dg-blink h-2 w-2 rounded-full bg-lime-400" aria-hidden="true" />
            <span className="text-xs font-semibold tracking-[0.08em] text-lime-400">LIVE</span>
            <span className="font-display text-sm font-semibold tabular-nums text-lavender-50">
              19:42
            </span>
          </div>
          <p className="text-right text-xs text-gray-300">
            Unit 3 · Elasticity of Demand · p. 57
          </p>
        </div>

        {/* Whiteboard content — this is what renders before the audio does */}
        <div className="mt-5 space-y-4">
          <h3 className="font-display text-lg font-semibold text-lavender-50">
            Elasticity of Demand
          </h3>
          <ul className="space-y-2 text-sm text-gray-300">
            <li className="flex gap-2">
              <span className="mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full bg-lime-400" />
              Demand is elastic when % change in quantity exceeds % change in price.
            </li>
            <li className="flex gap-2">
              <span className="mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full bg-lime-400" />
              Elasticity = %&Delta;Quantity ÷ %&Delta;Price
            </li>
          </ul>

          {/* Small hand-drawn-style demand curve, stroked in the annotation color */}
          <svg viewBox="0 0 200 90" className="h-20 w-full text-lime-400" aria-hidden="true">
            <path
              d="M10 12 V78 H190"
              fill="none"
              stroke="currentColor"
              strokeOpacity="0.35"
              strokeWidth="1.5"
            />
            <path
              d="M20 20 Q 90 40 180 68"
              fill="none"
              stroke="currentColor"
              strokeWidth="2.5"
              strokeLinecap="round"
            />
            <circle cx="180" cy="68" r="3.5" fill="currentColor" />
          </svg>
        </div>

        {/* Tutor voice indicator */}
        <div className="mt-5 flex items-center gap-3 border-t border-ink-800 pt-4">
          <span className="dg-pulse-ring flex h-6 w-6 items-center justify-center rounded-full bg-indigo-400">
            <span className="h-2.5 w-2.5 rounded-full bg-ink-950/40" />
          </span>
          <span className="text-sm text-gray-300">Tutor speaking</span>
          <span className="ml-auto flex h-4 items-end gap-0.5" aria-hidden="true">
            {[0, 0.15, 0.3, 0.1, 0.25].map((delay, i) => (
              <span
                key={i}
                className="dg-wave-bar h-full w-1 rounded-full bg-indigo-400"
                style={{ animationDelay: `${delay}s` }}
              />
            ))}
          </span>
        </div>
      </div>

      {/* One floating decorative badge — DESIGN.md caps this at 2–3 max; one is enough here */}
      <span
        aria-hidden="true"
        className="absolute -right-4 -top-4 flex h-11 w-11 items-center justify-center rounded-full bg-indigo-950 ring-1 ring-inset ring-indigo-400/30"
      >
        <IconSparkle className="h-5 w-5 text-indigo-300" />
      </span>
    </div>
  );
}

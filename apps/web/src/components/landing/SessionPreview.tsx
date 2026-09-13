/*
 * The hero visual: one real turn of a Digi Guru session.
 *
 * Digi Guru has no product screenshots yet and no photography to composite,
 * so rather than a stock image this draws the mechanism the whole product is
 * built on — the SyncGate. Board ops for turn N go out, the tutor's audio is
 * physically held, the client ACKs, the audio is released
 * (`.claude/rules/whiteboard-sync.md`). The numbers shown are the real budget
 * from that rule: a 400 ms hold ceiling, with a typical ACK well inside it.
 *
 * Everything here is decorative. The panel carries no accessible name of its
 * own; the hero copy beside it is what a screen reader reads.
 */

const boardLines = [
  "Demand is elastic when the % change in quantity exceeds the % change in price.",
  "Elasticity = %\u0394Quantity \u00f7 %\u0394Price",
];

export function SessionPreview() {
  return (
    <div aria-hidden="true" className="relative mx-auto w-full max-w-[30rem] lg:mx-0">
      {/* The board panel */}
      <div className="relative overflow-hidden rounded-md border border-ink-800 bg-ink-900 shadow-[0_40px_80px_-40px_rgba(0,0,0,0.9)]">
        {/* Inset top highlight — the one thing that reads the panel as raised */}
        <div className="pointer-events-none absolute inset-x-0 top-0 h-px bg-gradient-to-r from-transparent via-lavender-200/20 to-transparent" />

        {/* Chrome: session state on the left, citation on the right. The
            citation is always present — NN-4 means every turn has a source. */}
        <div className="flex items-center justify-between gap-4 border-b border-ink-800 px-5 py-3.5">
          <div className="flex items-center gap-2.5">
            <span className="dg-blink h-1.5 w-1.5 rounded-full bg-lime-400" />
            <span className="text-[10px] font-semibold tracking-[0.18em] text-lime-400">LIVE</span>
            <span className="font-display text-[13px] font-semibold tabular-nums text-gray-300">
              19:42
            </span>
          </div>
          <p className="truncate font-mono text-[11px] tracking-tight text-gray-500">
            unit&nbsp;3 · elasticity&nbsp;of&nbsp;demand · p.&nbsp;57
          </p>
        </div>

        {/* Board body */}
        <div className="px-5 py-6">
          <p className="text-[10px] font-semibold tracking-[0.18em] text-gray-500 uppercase">
            Whiteboard · turn 14
          </p>

          <h3 className="dg-ink-in mt-3 font-display text-xl font-semibold tracking-tight text-lavender-50">
            Elasticity of Demand
          </h3>

          <ul className="mt-4 space-y-3">
            {boardLines.map((line, i) => (
              <li
                key={line}
                className="dg-ink-in flex gap-3 text-[14px] leading-relaxed text-gray-300"
                style={{ animationDelay: `${0.18 + i * 0.18}s` }}
              >
                <span className="mt-[0.55rem] h-1 w-1 shrink-0 rounded-full bg-lime-400" />
                <span>{line}</span>
              </li>
            ))}
          </ul>

          {/* The demand curve, drawn as a board annotation would be */}
          <svg
            viewBox="0 0 220 104"
            className="dg-ink-in mt-6 h-[104px] w-full"
            style={{ animationDelay: "0.54s" }}
          >
            {/* axes */}
            <path
              d="M22 8 V88 H206"
              fill="none"
              stroke="currentColor"
              className="text-ink-700"
              strokeWidth="1.5"
            />
            {/* axis labels, set small like a real board annotation */}
            <text x="6" y="14" className="fill-gray-500" fontSize="9" fontFamily="monospace">
              P
            </text>
            <text x="202" y="101" className="fill-gray-500" fontSize="9" fontFamily="monospace">
              Q
            </text>
            {/* the curve */}
            <path
              d="M34 18 Q 108 46 196 80"
              fill="none"
              stroke="currentColor"
              className="text-lime-400"
              strokeWidth="2.25"
              strokeLinecap="round"
            />
            <circle cx="196" cy="80" r="3" className="fill-lime-400" />
            {/* the elastic segment, called out the way a tutor would */}
            <path
              d="M34 18 L 34 46 L 108 46"
              fill="none"
              stroke="currentColor"
              className="text-indigo-400"
              strokeWidth="1.25"
              strokeDasharray="3 4"
            />
          </svg>
        </div>

        {/* The gate. This is the product claim, stated as telemetry rather
            than as marketing copy: the board landed, then the voice opened. */}
        <div className="border-t border-ink-800 bg-ink-950/60 px-5 py-4">
          <div className="flex items-center justify-between text-[11px] font-medium tabular-nums">
            <span className="text-gray-300">board sent → ack</span>
            <span className="text-lime-400">148 ms</span>
          </div>

          <div className="mt-2 h-1 w-full overflow-hidden rounded-full bg-ink-800">
            {/* 148 of the 400 ms ceiling */}
            <div className="dg-hold-fill h-full w-[37%] rounded-full bg-lime-400" />
          </div>

          <div className="mt-2 flex items-center justify-between text-[10px] text-gray-500">
            <span>audio held until the board renders</span>
            <span className="font-mono">ceiling 400 ms</span>
          </div>
        </div>

        {/* Voice state */}
        <div className="flex items-center gap-3 border-t border-ink-800 px-5 py-3.5">
          <span className="dg-pulse-ring flex h-5 w-5 items-center justify-center rounded-full bg-indigo-400">
            <span className="h-2 w-2 rounded-full bg-ink-950/40" />
          </span>
          <span className="text-[13px] text-gray-300">Tutor speaking</span>
          <span className="ml-auto flex h-3.5 items-end gap-[3px]">
            {[0, 0.12, 0.28, 0.08, 0.2, 0.34].map((delay, i) => (
              <span
                key={i}
                className="dg-wave-bar h-full w-[3px] rounded-full bg-indigo-400/80"
                style={{ animationDelay: `${delay}s` }}
              />
            ))}
          </span>
        </div>
      </div>

      {/* Abstention, shown as a second card tucked under the first. It is the
          other half of the pitch (NN-4) and it is far more convincing shown
          as a real tutor turn than described in a feature bullet. */}
      <div className="mx-4 -mt-px rounded-b-md border border-t-0 border-ink-800 bg-ink-900/50 px-5 py-3.5">
        <p className="text-[10px] font-semibold tracking-[0.18em] text-gray-500 uppercase">
          Off-syllabus question
        </p>
        <p className="mt-1.5 text-[13px] leading-relaxed text-gray-300">
          &ldquo;That isn&rsquo;t covered in your textbook. The closest topic in this unit is{" "}
          <span className="text-lavender-50">price elasticity of supply</span> — shall we take
          that?&rdquo;
        </p>
      </div>
    </div>
  );
}

const steps = [
  {
    n: "01",
    title: "Pick your program",
    body: "Program, semester, and learner support centre are set once — Digi Guru remembers it every time you log back in.",
  },
  {
    n: "02",
    title: "Join a focused session",
    body: "Talk with your tutor live for up to 20 minutes. It only teaches from your own textbook — nothing else.",
  },
  {
    n: "03",
    title: "Pick up where you left off",
    body: "Next time, a 30–60 second recap gets you back to exactly where the last session ended.",
  },
];

// Numbered steps are legitimate here — this is a real, fixed sequence a
// student goes through, not decoration.
export function HowItWorks() {
  return (
    <section id="how-it-works" className="mx-auto max-w-6xl px-6 py-20 sm:py-28">
      <div className="max-w-xl">
        <p className="text-xs font-semibold tracking-[0.08em] text-indigo-300 uppercase">
          How a session works
        </p>
        <h2 className="mt-4 text-balance font-display text-3xl font-bold text-lavender-50 sm:text-4xl">
          From sign-in to session recap in three steps
        </h2>
      </div>

      <ol className="mt-14 grid gap-10 sm:grid-cols-3 sm:gap-8">
        {steps.map((step) => (
          <li key={step.n} className="border-t border-ink-800 pt-6">
            <span className="font-display text-sm font-semibold tabular-nums text-lime-400">
              {step.n}
            </span>
            <h3 className="mt-3 font-display text-lg font-semibold text-lavender-50">
              {step.title}
            </h3>
            <p className="mt-2 text-[15px] leading-relaxed text-gray-300">{step.body}</p>
          </li>
        ))}
      </ol>
    </section>
  );
}

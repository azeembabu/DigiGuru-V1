import Link from "next/link";
import { NavBar } from "./components/NavBar";

const STATS = [
  { value: "20 min", label: "Focused daily voice lessons" },
  { value: "1:1", label: "AI tutor, every session" },
  { value: "100%", label: "Bound to your syllabus" },
];

const FEATURES = [
  {
    title: "Live voice tutoring",
    body: "A real two-way conversation with your AI tutor — ask, get corrected, ask again, exactly like a human class.",
  },
  {
    title: "Whiteboard-first",
    body: "Every explanation is written on the whiteboard before it's spoken, so you always see it before you hear it.",
  },
  {
    title: "Strictly your syllabus",
    body: "The tutor only answers from your program's own textbook content — no drifting into unrelated topics.",
  },
];

export default function Home() {
  return (
    <div className="flex min-h-screen flex-col bg-ink-950">
      <NavBar />

      {/* Hero — DESIGN.md §7 row 1 (dark hero), §7.1 (mobile: single column,
          stacked headline -> stats -> CTA -> portrait; side-by-side only at lg:) */}
      <section className="relative mx-auto flex w-full max-w-7xl flex-1 flex-col items-center gap-12 px-4 pb-16 pt-8 sm:px-6 sm:pb-24 lg:flex-row lg:items-center lg:gap-16 lg:px-8 lg:pt-4">
        <div className="flex flex-col items-center text-center lg:w-1/2 lg:items-start lg:text-left">
          <p className="mb-4 font-display text-[11px] font-medium uppercase tracking-[0.2em] text-indigo-300">
            AI-powered e-learning
          </p>

          <h1 className="max-w-xl font-display text-[clamp(32px,9vw,56px)] font-bold leading-[1.1] text-white lg:leading-[1.05]">
            Learn faster with an AI tutor that&nbsp;
            <span className="text-lime-400">talks with you</span>
          </h1>

          <p className="mt-6 max-w-md text-[18px] leading-relaxed text-dg-gray-300">
            Digi Guru replaces a human tutor for your fixed syllabus — live voice lessons, a
            whiteboard that stays a step ahead, and zero drift outside your textbook.
          </p>

          <div className="mt-8 flex flex-col gap-4 sm:flex-row">
            <Link
              href="/signup"
              className="group relative inline-flex min-h-12 items-center justify-center rounded-full bg-lime-400 px-8 py-3 text-base font-semibold text-ink-950 transition hover:bg-lime-300 focus-visible:outline-2 focus-visible:outline-indigo-300 focus-visible:outline-offset-2"
            >
              <span
                aria-hidden
                className="absolute inset-0 -z-10 rounded-full bg-lime-400 opacity-0 blur-xl transition group-hover:opacity-35"
              />
              Get Started
            </Link>
            <a
              href="#how-it-works"
              className="inline-flex min-h-12 items-center justify-center gap-1.5 rounded-full px-8 py-3 text-base font-semibold text-white underline-offset-4 transition hover:underline"
            >
              Learn More
              <svg
                aria-hidden
                viewBox="0 0 24 24"
                width="16"
                height="16"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                className="transition group-hover:rotate-45"
              >
                <path d="M7 17L17 7M17 7H8M17 7v9" />
              </svg>
            </a>
          </div>

          {/* Stat row — desktop: 3-across; mobile: horizontal scroll-snap
              per DESIGN.md §7.1 (never stack vertically, triples hero height) */}
          <div className="mt-12 flex w-full snap-x snap-mandatory gap-8 overflow-x-auto pb-2 lg:w-auto lg:overflow-visible lg:pb-0">
            {STATS.map((stat) => (
              <div key={stat.label} className="flex shrink-0 snap-start flex-col">
                <span className="font-display text-2xl font-bold tabular-nums text-white">
                  {stat.value}
                </span>
                <span className="mt-1 max-w-[140px] text-sm text-dg-gray-300">
                  {stat.label}
                </span>
              </div>
            ))}
          </div>
        </div>

        {/* Portrait glow composition — hidden below lg per DESIGN.md §7.1
            (competes with limited width, adds no information on mobile) */}
        <div className="relative hidden aspect-square w-full max-w-md items-center justify-center lg:flex lg:w-1/2">
          <div
            aria-hidden
            className="absolute inset-0 rounded-[28px]"
            style={{
              background:
                "radial-gradient(120% 120% at 30% 20%, #6E74E8 0%, #3E3FA8 55%, #171A33 100%)",
            }}
          />
          <div className="relative flex h-4/5 w-4/5 items-center justify-center rounded-[28px] border border-ink-700 bg-ink-900/60 backdrop-blur-sm">
            <span className="font-display text-sm text-dg-gray-300">
              [ classroom preview ]
            </span>
          </div>
          <div
            aria-hidden
            className="absolute -left-4 top-8 flex h-14 w-14 items-center justify-center rounded-full bg-indigo-950 text-indigo-400 shadow-lg"
          >
            <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M13 2L3 14h7l-1 8 11-14h-7l1-6z" />
            </svg>
          </div>
        </div>
      </section>

      {/* Features — DESIGN.md §7.1: 1 col below sm:, 2 at sm:, 3+ at lg: */}
      <section id="features" className="mx-auto w-full max-w-7xl px-4 py-16 sm:px-6 sm:py-24 lg:px-8">
        <h2 className="mx-auto max-w-2xl text-center font-display text-[clamp(26px,6vw,40px)] font-bold leading-[1.15] text-white">
          Everything a{" "}
          <span className="text-lime-400">real tutor</span> would do, on demand
        </h2>

        <div className="mt-12 grid grid-cols-1 gap-6 sm:grid-cols-2 lg:grid-cols-3">
          {FEATURES.map((feature) => (
            <div
              key={feature.title}
              className="rounded-2xl border border-ink-800 bg-ink-900 p-6"
            >
              <h3 className="font-display text-xl font-bold text-white">
                {feature.title}
              </h3>
              <p className="mt-2 text-[16px] leading-relaxed text-dg-gray-300">
                {feature.body}
              </p>
            </div>
          ))}
        </div>
      </section>

      <section id="how-it-works" className="mx-auto w-full max-w-3xl px-4 py-16 text-center sm:px-6 sm:py-24 lg:px-8">
        <h2 className="font-display text-[clamp(26px,6vw,40px)] font-bold leading-[1.15] text-white">
          Log in, and your <span className="text-lime-400">next lesson</span> starts itself
        </h2>
        <p className="mt-4 text-[16px] leading-relaxed text-dg-gray-300">
          Digi Guru remembers exactly where you left off — program, semester, and block — so class
          starts on the right page every time.
        </p>
      </section>

      <section id="testimonials" className="mx-auto w-full max-w-3xl px-4 pb-16 text-center sm:px-6 sm:pb-24 lg:px-8">
        <div className="mx-auto flex max-w-md flex-col items-center gap-3 rounded-2xl border border-ink-800 bg-ink-900 p-8">
          <div aria-hidden className="flex gap-1 text-lime-400">
            {"★★★★★".split("").map((star, i) => (
              <span key={i}>{star}</span>
            ))}
          </div>
          <p className="text-[16px] leading-relaxed text-dg-gray-300">
            &ldquo;It feels like the tutor is actually watching the same page I am.&rdquo;
          </p>
        </div>
      </section>

      <footer className="border-t border-ink-800 px-4 py-8 text-center text-sm text-dg-gray-500 sm:px-6 lg:px-8">
        © {new Date().getFullYear()} Digi Guru
      </footer>
    </div>
  );
}

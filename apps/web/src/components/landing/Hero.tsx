import { Button } from "@/components/ui/Button";
import { SessionPreview } from "@/components/landing/SessionPreview";
import { IconArrowUpRight } from "@/components/icons";

const stats = [
  { value: "20 min", label: "Focused session length" },
  { value: "100%", label: "Answers from your syllabus" },
  { value: "2", label: "Languages — English & Malayalam" },
];

export function Hero() {
  return (
    <section id="top" className="mx-auto max-w-6xl px-6 pt-16 pb-20 sm:pt-24 sm:pb-28">
      <div className="grid items-center gap-16 lg:grid-cols-[1.05fr_1fr]">
        <div>
          <p className="text-xs font-semibold tracking-[0.08em] text-indigo-300 uppercase">
            Live voice tutoring
          </p>

          <h1 className="mt-4 text-balance font-display text-4xl leading-[1.08] font-bold text-lavender-50 sm:text-5xl lg:text-[3.25rem]">
            A voice tutor that writes
            <br />
            <span className="text-lime-400">before it speaks.</span>
          </h1>

          <p className="mt-6 max-w-md text-lg leading-relaxed text-gray-300">
            Digi Guru teaches live, over voice, strictly from your own textbook — and puts every
            explanation on the whiteboard before it says a word.
          </p>

          <div className="mt-8 flex flex-wrap items-center gap-x-6 gap-y-4">
            <Button href="#cta" variant="primary">
              Start a session
            </Button>
            <Button href="#how-it-works" variant="ghost" className="group">
              See how it works
              <IconArrowUpRight className="h-4 w-4 transition-transform group-hover:rotate-45" />
            </Button>
          </div>

          <dl className="mt-14 grid grid-cols-3 gap-6 border-t border-ink-800 pt-8">
            {stats.map((stat) => (
              <div key={stat.label}>
                <dt className="sr-only">{stat.label}</dt>
                <dd className="font-display text-2xl font-bold tabular-nums text-lavender-50 sm:text-3xl">
                  {stat.value}
                </dd>
                <p className="mt-1 text-sm text-gray-300">{stat.label}</p>
              </div>
            ))}
          </dl>
        </div>

        <SessionPreview />
      </div>
    </section>
  );
}

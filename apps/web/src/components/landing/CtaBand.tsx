import { Button } from "@/components/ui/Button";

export function CtaBand() {
  return (
    <section id="cta" className="mx-auto max-w-6xl px-6 py-20 sm:py-28">
      <div className="relative overflow-hidden rounded-lg border border-ink-800 bg-ink-900 px-8 py-14 text-center sm:px-16">
        <div
          aria-hidden="true"
          className="absolute -top-24 left-1/2 -z-0 h-64 w-64 -translate-x-1/2 rounded-full opacity-40 blur-3xl"
          style={{ background: "radial-gradient(closest-side, #6E74E8, transparent)" }}
        />
        <div className="relative">
          <h2 className="text-balance font-display text-3xl font-bold text-lavender-50 sm:text-4xl">
            Ready to see your syllabus taught live?
          </h2>
          <p className="mx-auto mt-4 max-w-md text-[15px] text-gray-300">
            Built for open-university learners — sign in with your roll number and learner support
            centre to get started.
          </p>
          <div className="mt-8 flex justify-center">
            <Button href="/signup" variant="primary">
              Start a session
            </Button>
          </div>
        </div>
      </div>
    </section>
  );
}

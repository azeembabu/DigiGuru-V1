import { Card, CardLabel } from "@/components/dashboard/Card";
import { Button } from "@/components/ui/Button";
import { IconPlay } from "@/components/icons";
import type { StudentContext } from "@/lib/student-context";

export function ContinueCard({ context }: { context: StudentContext }) {
  const block = context.current_block;

  // No block means the classroom has nothing to open on — a session would fail
  // at `session_init`. So the empty state deliberately offers no CTA at all
  // rather than a disabled one that invites clicking.
  if (block === null) {
    return (
      <Card className="sm:!p-8">
        <CardLabel>Your classroom</CardLabel>
        <h2 className="mt-3 text-balance font-display text-[28px] font-bold leading-[1.2] text-white">
          No block assigned yet
        </h2>
        <p className="mt-3 max-w-xl text-[16px] leading-[1.6] text-gray-300">
          A coordinator at your learning centre assigns your first block. Once they do, it appears
          here and you can start your first session straight from this page.
        </p>
      </Card>
    );
  }

  return (
    <Card className="sm:!p-8">
      <CardLabel>Continue learning</CardLabel>
      {/* Two-tone headline (DESIGN.md §2): one highlighted phrase. On this dark
          surface the highlight is `lime-400` — the `lime-500` rule applies only
          to lime text on a light ground (§8). */}
      <h2 className="mt-3 text-balance font-display text-[28px] font-bold leading-[1.2] text-white">
        Pick up where you <span className="text-lime-400">left off</span>
      </h2>
      <p className="mt-3 font-display text-[20px] font-bold leading-[1.3] text-white">
        {block.title}
      </p>
      <p className="mt-1 text-[14px] leading-[1.5] text-gray-300/70">Block {block.block_no}</p>

      {/* DESIGN.md §1.2 "CTA glow": the blurred lime ellipse is opacity-0 at rest
          and only warms up on hover or keyboard focus, so it never reads as a
          second static surface behind the button. `focus-within` carries the
          keyboard case because the focusable node is the inner <Link>. */}
      <div className="group relative mt-7 inline-flex">
        <span
          aria-hidden="true"
          className="pointer-events-none absolute -inset-x-6 -inset-y-4 rounded-full bg-lime-400 opacity-0 blur-[40px] transition-opacity duration-300 group-hover:opacity-35 group-focus-within:opacity-35"
        />
        <Button href="/classroom" variant="primary" className="relative">
          <IconPlay className="h-5 w-5" />
          Start session
        </Button>
      </div>

      <p className="mt-4 text-[14px] leading-[1.5] text-gray-300/70">
        Sessions run for up to 20 minutes of speaking time.
      </p>
    </Card>
  );
}

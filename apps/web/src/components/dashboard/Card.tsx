import type { ReactNode } from "react";

// The dashboard runs on the green-black surface sampled from the auth key art
// (`globals.css`, the `art-*` tokens), so signing in does not throw the student
// from a dark page onto a light one. That is a deliberate departure from
// DESIGN.md §7, which specifies light mode here — raise it before extending the
// pattern to the admin console, which is still light.
//
// Card construction follows DESIGN.md §5.4's dark rule: a flat elevated fill, a
// 1px border, and the §1.2 edge glow (a hairline white top highlight) instead
// of a drop shadow. Elevation on dark is light, not shadow.
export const cardClass =
  "relative rounded-[16px] border border-art-mid/60 bg-art-deep/70 p-5 sm:p-6 " +
  "before:pointer-events-none before:absolute before:inset-x-0 before:top-0 before:h-px " +
  "before:rounded-t-[16px] before:bg-white/[0.06]";

export function Card({ className = "", children }: { className?: string; children: ReactNode }) {
  return <section className={`${cardClass} ${className}`}>{children}</section>;
}

/** The 12px uppercase eyebrow from the type scale (DESIGN.md §2). */
export function CardLabel({ children }: { children: ReactNode }) {
  return (
    <p className="text-[12px] font-semibold uppercase leading-[1.4] tracking-[0.04em] text-gray-300/70">
      {children}
    </p>
  );
}

export function CardTitle({ children }: { children: ReactNode }) {
  return <h2 className="font-display text-[20px] font-bold leading-[1.3] text-white">{children}</h2>;
}

/**
 * A stat tile — the compact metric card the reference layout leads with.
 *
 * `value` is deliberately a string the caller has already resolved: there is no
 * number here the UI is allowed to compute or estimate. Every tile on this page
 * shows something the gateway actually returned, or the fixed 20-minute cap
 * (NN-3). An empty state passes an em-dash, never a zero.
 */
export function StatTile({
  label,
  value,
  hint,
  icon,
}: {
  label: string;
  value: string;
  hint?: string;
  icon?: ReactNode;
}) {
  return (
    <div className={`${cardClass} !p-5`}>
      <div className="flex items-start justify-between gap-3">
        <p className="text-[13px] leading-[1.5] text-gray-300/70">{label}</p>
        {icon ? <span className="shrink-0 text-art-glow">{icon}</span> : null}
      </div>
      <p className="mt-2 font-display text-[26px] font-bold leading-[1.15] tabular-nums text-white">
        {value}
      </p>
      {hint ? <p className="mt-1 text-[13px] leading-[1.5] text-gray-300/60">{hint}</p> : null}
    </div>
  );
}

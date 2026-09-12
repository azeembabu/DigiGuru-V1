import Link from "next/link";
import type { ReactNode } from "react";

// DESIGN.md §5.1. The `-light` variants are the same roles on the app's
// light surfaces (dashboard, admin): lime-500 rather than lime-400, because
// lime-400 does not hold AA against `ink-950` text on a `lavender-50` ground,
// and the focus ring offset has to match the surface it sits on — an
// `ink-950` offset on a light page reads as a dark halo, not a gap.
export type Variant = "primary" | "outline" | "ghost" | "primary-light" | "outline-light";

const base =
  "inline-flex items-center justify-center gap-2 rounded-full font-sans text-[15px] font-semibold " +
  "transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-300 " +
  "focus-visible:ring-offset-2";

const variants: Record<Variant, string> = {
  // Fill lime-400, text ink-950 — never white on lime (fails contrast). DESIGN.md §5.1.
  primary: "bg-lime-400 text-ink-950 px-6 py-3 hover:bg-lime-300 focus-visible:ring-offset-ink-950",
  outline:
    "border-[1.5px] border-lime-400 text-lime-400 px-6 py-2.5 hover:bg-lime-400/10 focus-visible:ring-offset-ink-950",
  ghost:
    "text-lavender-50 underline decoration-gray-500 underline-offset-4 hover:decoration-lime-400 focus-visible:ring-offset-ink-950",
  "primary-light":
    "bg-lime-500 text-ink-950 px-6 py-3 hover:bg-lime-400 focus-visible:ring-offset-lavender-50",
  "outline-light":
    "border-[1.5px] border-lavender-200 text-gray-900 px-6 py-2.5 hover:bg-white focus-visible:ring-offset-lavender-50",
};

/** Shared by `Button` and the form submit button, so both stay on one spec. */
export function buttonClass(variant: Variant = "primary", className = ""): string {
  return `${base} ${variants[variant]} ${className}`;
}

export function Button({
  href,
  variant = "primary",
  children,
  className = "",
}: {
  href: string;
  variant?: Variant;
  children: ReactNode;
  className?: string;
}) {
  return (
    <Link href={href} className={buttonClass(variant, className)}>
      {children}
    </Link>
  );
}

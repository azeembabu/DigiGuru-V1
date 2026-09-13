// Surface primitives for the quiet-luxury dashboard.
//
// Two card families, because this screen is the one place in the product that
// mixes surfaces: `lightCard` on the cream content column, `darkCard` for the
// two contrast panels (the speaking-time KPI and the weak-topic widget). Both
// are soft-radius with a hairline border and a low-spread shadow — on the dark
// card the elevation is a hairline top highlight rather than a shadow, since a
// drop shadow against charcoal reads as a smudge.

import type { ReactNode } from "react";

export const lightCard =
  "rounded-[18px] border border-lux-cream-300 bg-white/70 shadow-[0_1px_2px_rgba(28,27,24,0.04),0_8px_24px_-16px_rgba(28,27,24,0.18)]";

export const darkCard =
  "relative overflow-hidden rounded-[18px] border border-lux-char-700 bg-lux-char-900 " +
  "before:pointer-events-none before:absolute before:inset-x-0 before:top-0 before:h-px before:bg-white/10";

/** The 11px uppercase eyebrow that labels every card. */
export function Eyebrow({ dark = false, children }: { dark?: boolean; children: ReactNode }) {
  return (
    <p
      className={`text-[11px] font-semibold uppercase leading-[1.4] tracking-[0.1em] ${
        dark ? "text-lux-mist-400" : "text-lux-ink-600"
      }`}
    >
      {children}
    </p>
  );
}

/**
 * A section's heading.
 *
 * `level` is explicit rather than inferred: the page's heading order is a real
 * outline (h1 "Dashboard" in the header, h2 per region, h3 inside a region) and
 * a component that always emitted `h2` would silently break it.
 */
export function SectionHeading({
  level = 2,
  dark = false,
  id,
  children,
}: {
  level?: 2 | 3;
  dark?: boolean;
  id?: string;
  children: ReactNode;
}) {
  const Tag = level === 2 ? "h2" : "h3";
  return (
    <Tag
      id={id}
      className={`font-display text-[17px] font-bold leading-[1.3] ${
        dark ? "text-lux-mist-100" : "text-lux-ink-900"
      }`}
    >
      {children}
    </Tag>
  );
}

/** Supporting line under a heading. Both steps hold 4.5:1 on their surface. */
export function SubText({ dark = false, children }: { dark?: boolean; children: ReactNode }) {
  return (
    <p
      className={`text-[13.5px] leading-[1.6] ${dark ? "text-lux-mist-400" : "text-lux-ink-600"}`}
    >
      {children}
    </p>
  );
}

// --------------------------------------------------------------------- value

/**
 * A KPI tile's number.
 *
 * Proportional figures, not `tabular-nums`: at 34px the tabular variant gives
 * every digit the width of a zero and a number like "11" reads loose. Tabular is
 * reserved for the table's columns, where vertical alignment is the point.
 */
export function BigValue({ dark = false, children }: { dark?: boolean; children: ReactNode }) {
  return (
    <p
      className={`font-display text-[34px] font-bold leading-[1.05] ${
        dark ? "text-lux-mist-100" : "text-lux-ink-900"
      }`}
    >
      {children}
    </p>
  );
}

// --------------------------------------------------------------------- badge

export type BadgeTone = "neutral" | "good" | "gold" | "warn";

/**
 * A small status badge.
 *
 * Every tone ships a word, never a bare colour — the dashboard has to survive
 * forced-colours mode and a monochrome reader, and the muted gold/green accents
 * are close enough in lightness that hue alone would be a weak signal even for
 * full-colour vision.
 */
export function Badge({
  tone = "neutral",
  dark = false,
  children,
}: {
  tone?: BadgeTone;
  dark?: boolean;
  children: ReactNode;
}) {
  const light: Record<BadgeTone, string> = {
    neutral: "border-lux-cream-400 bg-lux-cream-200 text-lux-ink-700",
    good: "border-[#0a6b3c]/30 bg-[#0a6b3c]/10 text-[#0a6b3c]",
    gold: "border-[#9c7a24]/35 bg-[#9c7a24]/10 text-[#7d6019]",
    warn: "border-[#a33226]/30 bg-[#a33226]/10 text-[#a33226]",
  };
  const darkTones: Record<BadgeTone, string> = {
    neutral: "border-lux-char-600 bg-lux-char-800 text-lux-mist-300",
    good: "border-[#35a873]/40 bg-[#35a873]/15 text-[#6fd3a1]",
    gold: "border-[#b8851f]/45 bg-[#b8851f]/15 text-[#ddb463]",
    warn: "border-[#ec835a]/45 bg-[#ec835a]/15 text-[#f0a182]",
  };

  return (
    <span
      className={`inline-flex items-center gap-1 rounded-full border px-2.5 py-0.5 text-[12px] font-semibold leading-[1.5] ${
        dark ? darkTones[tone] : light[tone]
      }`}
    >
      {children}
    </span>
  );
}

// -------------------------------------------------------------------- button

const buttonBase =
  "inline-flex items-center justify-center gap-2 rounded-full font-sans text-[14.5px] font-semibold " +
  "transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#9c7a24] " +
  "focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-60";

/**
 * The dashboard's buttons.
 *
 * `primary` is charcoal-on-cream rather than gold-on-cream: the gold step that
 * passes as a chart mark does not hold 4.5:1 behind white text, and darkening it
 * far enough to do so stops looking like the accent. Gold stays a chart and badge
 * colour; the CTA gets the charcoal the rail already establishes.
 */
export function luxButton(
  variant: "primary" | "quiet" | "onDark" = "primary",
  className = "",
): string {
  const variants = {
    primary:
      "bg-lux-char-900 px-5 py-2.5 text-lux-mist-100 hover:bg-lux-char-800 focus-visible:ring-offset-lux-cream-100",
    quiet:
      "border border-lux-cream-400 bg-white px-4 py-2 text-lux-ink-900 hover:bg-lux-cream-200 focus-visible:ring-offset-lux-cream-100",
    onDark:
      "border border-lux-char-600 bg-lux-char-800 px-4 py-2 text-lux-mist-100 hover:bg-lux-char-700 focus-visible:ring-offset-lux-char-900",
  } as const;

  return `${buttonBase} ${variants[variant]} ${className}`;
}

// ------------------------------------------------------------------ skeleton

/**
 * A loading placeholder.
 *
 * `aria-hidden` with the busy state announced once on the region that owns it —
 * a dozen pulsing blocks each announcing themselves is noise, and the region's
 * `aria-busy` already says the real thing.
 */
export function Skeleton({ className = "h-24" }: { className?: string }) {
  return (
    <div className={`animate-pulse rounded-[18px] bg-lux-cream-300/70 ${className}`} aria-hidden="true" />
  );
}

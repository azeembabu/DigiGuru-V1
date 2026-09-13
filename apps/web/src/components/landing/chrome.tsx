import type { ReactNode } from "react";

/*
 * The landing page's shared frame.
 *
 * Every section on `/` is built from these three pieces rather than each one
 * re-inventing its own padding and heading rhythm. The page reads as one
 * document because the measure, the gutter, and the eyebrow-to-heading
 * spacing are defined exactly once, here.
 */

/** The page's single content measure. Nothing on `/` sets its own max-width. */
export function Shell({ children, className = "" }: { children: ReactNode; className?: string }) {
  return <div className={`mx-auto w-full max-w-[1180px] px-6 lg:px-10 ${className}`}>{children}</div>;
}

/**
 * A full-bleed hairline separating two sections. Drawn as its own element
 * rather than a `border-t` on the section so it can run edge to edge while the
 * content inside stays on the measure — the horizontal rules are the page's
 * structure, so they are not allowed to stop at the gutter.
 */
export function Rule() {
  return <div aria-hidden="true" className="h-px w-full bg-ink-800" />;
}

/**
 * Eyebrow + heading + optional lede. The lime tick before the eyebrow is the
 * one repeating ornament on the page; it is what makes a section start
 * legible at a glance when scrolling past.
 */
export function SectionHead({
  eyebrow,
  title,
  lede,
  align = "left",
}: {
  eyebrow: string;
  title: ReactNode;
  lede?: string;
  align?: "left" | "center";
}) {
  const centered = align === "center";
  return (
    <div className={centered ? "mx-auto max-w-2xl text-center" : "max-w-2xl"}>
      <p
        className={`flex items-center gap-3 text-[11px] font-semibold tracking-[0.18em] text-gray-300 uppercase ${
          centered ? "justify-center" : ""
        }`}
      >
        <span aria-hidden="true" className="h-px w-6 bg-lime-400" />
        {eyebrow}
      </p>
      <h2 className="mt-5 text-balance font-display text-[2rem] leading-[1.12] font-bold tracking-tight text-lavender-50 sm:text-[2.6rem]">
        {title}
      </h2>
      {lede ? (
        <p className={`mt-5 text-[17px] leading-relaxed text-gray-300 ${centered ? "" : "max-w-xl"}`}>
          {lede}
        </p>
      ) : null}
    </div>
  );
}

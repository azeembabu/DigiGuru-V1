import Link from "next/link";
import type { ComponentType, ReactNode } from "react";

import keyArt from "@/../public/brand/auth-key-art.webp";
import { Logo } from "@/components/ui/Logo";

// Auth split layout: the key art fills the left half of the viewport, with all
// of its copy rendered as real HTML on top of it. DESIGN.md §7.
//
// Two deliberate choices here:
//
// 1. The photograph is a CSS *background*, not an `<img>`. That keeps it out
//    of the document's image list, so there is no right-click "Save image as",
//    no drag-to-desktop, and no long-press save on touch. It is a deterrent,
//    not protection — the file is still a normal network request and anyone
//    who opens devtools can fetch it — but it stops casual copying.
//
// 2. The artwork's own typeset copy is masked, not used. The image is anchored
//    right so the photographic side (desk, mug, laptop, foliage) fills the
//    frame, and a hard left-to-right scrim covers the region where the baked
//    headline and benefit text sit. Those words are then re-rendered as HTML
//    over the scrim, so they reflow, scale with the reader's font size, can be
//    translated, and reach a screen reader.
//
// Layout contract:
// - Mobile: single column. The art is dropped entirely — a ~107KB decorative
//   asset, and this audience is on metered mobile data
//   (IMPLEMENTATION_PLAN.md §9). The logo rides in the header instead.
// - Desktop (lg+): a 50/50 grid pinned to the viewport (`lg:h-dvh`), so the
//   window itself never scrolls. If a short viewport cannot fit the form, the
//   right column scrolls on its own rather than clipping.

export type AuthFeature = {
  icon: ComponentType<{ className?: string }>;
  title: string;
  body: string;
};

export function AuthShell({
  eyebrow,
  title,
  highlight,
  blurb,
  features,
  tagline,
  footnote,
  children,
}: {
  /** Rendered as a spaced, dot-separated rule — e.g. ["Learn","Interact","Grow"]. */
  eyebrow: string[];
  title: string;
  /** The single lime-emphasised phrase — DESIGN.md §2 allows exactly one. */
  highlight: string;
  blurb: string;
  features: AuthFeature[];
  tagline: string;
  footnote: string[];
  children: ReactNode;
}) {
  return (
    <div className="min-h-dvh bg-art-base lg:grid lg:h-dvh lg:grid-cols-2 lg:overflow-hidden">
      {/* Left half — artwork, with its copy re-rendered as HTML on top. */}
      <aside className="relative hidden select-none overflow-hidden bg-art-base lg:flex lg:flex-col">
        {/* The photograph. `bg-right` keeps the desk scene in frame; `bg-cover`
            fills the panel at any aspect, so there is never a letterbox. */}
        <div
          aria-hidden="true"
          className="pointer-events-none absolute inset-0 bg-cover bg-right bg-no-repeat"
          style={{ backgroundImage: `url(${keyArt.src})` }}
        />

        {/* Scrim: opaque over the baked-in text, clearing toward the desk. */}
        <div
          aria-hidden="true"
          className="pointer-events-none absolute inset-0"
          style={{
            backgroundImage: [
              "linear-gradient(90deg, rgba(5,9,12,0.99) 0%, rgba(5,9,12,0.97) 46%, rgba(5,9,12,0.62) 68%, rgba(5,9,12,0.30) 100%)",
              "linear-gradient(180deg, rgba(5,9,12,0.55) 0%, rgba(5,9,12,0) 28%, rgba(5,9,12,0.45) 100%)",
            ].join(", "),
          }}
        />

        {/* A faint glow that rises and falls. Opacity only, and motion-reduced
            by the global rule in globals.css. */}
        <div
          aria-hidden="true"
          className="dg-art-glow pointer-events-none absolute inset-0 mix-blend-screen"
          style={{
            background:
              "radial-gradient(52% 40% at 72% 52%, rgba(120,230,150,0.20) 0%, rgba(10,12,22,0) 72%)",
          }}
        />

        <div className="relative flex min-h-0 flex-1 flex-col justify-center px-10 py-8 xl:px-14">
          <Link href="/" aria-label="Digi Guru — home" className="inline-block w-fit">
            <Logo size="md" />
          </Link>

          <div className="mt-6">
            <p className="flex items-center gap-2.5 text-[11px] font-semibold tracking-[0.24em] text-gray-300 uppercase">
              {eyebrow.map((word, i) => (
                <span key={word} className="flex items-center gap-2.5">
                  {i > 0 ? (
                    <span aria-hidden="true" className="text-lime-400">
                      &bull;
                    </span>
                  ) : null}
                  {word}
                </span>
              ))}
            </p>
            <span aria-hidden="true" className="mt-2 block h-0.5 w-20 rounded-full bg-lime-400" />
          </div>

          <h1 className="mt-5 text-balance font-display text-[2.1rem] leading-[1.1] font-bold text-white xl:text-[2.6rem]">
            {title}
            <br />
            <span className="text-lime-400">{highlight}</span>
          </h1>

          <p className="mt-4 max-w-md text-[15px] leading-relaxed text-gray-300">{blurb}</p>

          <ul className="mt-7 space-y-5">
            {features.map(({ icon: Icon, title: name, body }) => (
              <li key={name} className="flex gap-4">
                <span
                  aria-hidden="true"
                  className="flex h-12 w-12 shrink-0 items-center justify-center rounded-full border border-lime-400/50 bg-lime-400/[0.08] shadow-[0_0_22px_rgba(166,230,53,0.16)]"
                >
                  <Icon className="h-5 w-5 text-lime-300" />
                </span>
                <div className="pt-0.5">
                  <h2 className="font-display text-[15px] font-semibold text-white">{name}</h2>
                  <p className="mt-1 max-w-sm text-sm leading-relaxed text-gray-300">{body}</p>
                </div>
              </li>
            ))}
          </ul>

          <div className="mt-7">
            <p className="font-display text-2xl leading-tight font-semibold text-lime-300 italic">
              {tagline}
            </p>
            {/* The underline flourish under the tagline in the comp. */}
            <svg
              aria-hidden="true"
              viewBox="0 0 240 12"
              className="mt-1 h-3 w-56 text-lime-400"
              fill="none"
            >
              <path
                d="M2 8.5C46 3.5 150 1.5 238 3.5"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
              />
            </svg>
          </div>

          <p className="mt-6 flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] font-medium tracking-[0.2em] text-gray-300 uppercase">
            {footnote.map((item, i) => (
              <span key={item} className="flex items-center gap-3">
                {i > 0 ? (
                  <span aria-hidden="true" className="text-gray-500">
                    |
                  </span>
                ) : null}
                {item}
              </span>
            ))}
          </p>
        </div>
      </aside>

      {/* Right half — the form surface, carrying the artwork's palette. */}
      <div
        className="relative flex min-h-dvh flex-col lg:h-dvh lg:min-h-0 lg:overflow-y-auto"
        style={{
          // Continues the artwork's light across the seam: brightest at the
          // top-left where the halves meet, decaying to near-black, the same
          // way the photograph does. Colours are sampled from the image — see
          // the `--color-art-*` tokens in globals.css.
          backgroundImage: [
            "radial-gradient(70% 55% at 2% 6%, rgba(96,157,79,0.28) 0%, rgba(25,52,37,0.13) 45%, rgba(5,9,12,0) 78%)",
            "radial-gradient(90% 70% at 78% 22%, rgba(79,233,85,0.09) 0%, rgba(5,9,12,0) 70%)",
            "linear-gradient(180deg, rgba(11,26,18,0.55) 0%, rgba(5,9,12,1) 72%)",
          ].join(", "),
        }}
      >
        <header className="flex shrink-0 items-center justify-between px-6 py-4 sm:px-10">
          <Link href="/" className="lg:invisible" aria-label="Digi Guru — home">
            <Logo size="sm" />
          </Link>
          <Link
            href="/"
            className="rounded-sm text-sm text-gray-300 transition-colors hover:text-lavender-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-300 focus-visible:ring-offset-2 focus-visible:ring-offset-art-base"
          >
            &larr; Back to home
          </Link>
        </header>

        <main className="flex flex-1 items-center px-6 pb-10 sm:px-10 lg:pb-6">
          <div className="mx-auto w-full max-w-[30rem]">{children}</div>
        </main>
      </div>
    </div>
  );
}

/**
 * The form block. No card chrome: on desktop the right half of the split IS
 * the surface, so nesting a bordered panel inside it just drew a box within a
 * box. The heading/divider structure is kept.
 */
export function AuthCard({
  heading,
  sub,
  children,
  footer,
}: {
  heading: string;
  sub: string;
  children: ReactNode;
  footer: ReactNode;
}) {
  return (
    <div>
      <h2 className="font-display text-2xl font-bold text-lavender-50 sm:text-3xl">{heading}</h2>
      <p className="mt-2 text-sm leading-relaxed text-gray-300">{sub}</p>
      <div className="mt-6">{children}</div>
      <div className="mt-6 border-t border-ink-800 pt-5 text-center text-sm text-gray-300">
        {footer}
      </div>
    </div>
  );
}

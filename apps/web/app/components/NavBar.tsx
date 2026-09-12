"use client";

import { useState } from "react";
import Link from "next/link";

const LINKS = [
  { href: "#features", label: "Features" },
  { href: "#how-it-works", label: "How It Works" },
  { href: "#testimonials", label: "Testimonials" },
];

/**
 * DESIGN.md §5.3 (nav) + §7.1 (mobile): horizontal link row collapses to a
 * hamburger below `lg:`, opening a full-height dark overlay panel with the
 * same links stacked plus the CTA pair pinned to the bottom.
 */
export function NavBar() {
  const [open, setOpen] = useState(false);

  return (
    <header className="relative z-20 mx-auto flex max-w-7xl items-center justify-between px-4 py-5 sm:px-6 lg:px-8">
      <Link href="/" className="flex items-center gap-2.5 rounded-xl focus-visible:outline-2 focus-visible:outline-indigo-300 focus-visible:outline-offset-2">
        <span
          aria-hidden
          className="flex h-9 w-9 items-center justify-center rounded-full border-2 border-lime-400 text-lime-400"
        >
          <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="2">
            <circle cx="12" cy="12" r="9" />
          </svg>
        </span>
        <span className="font-display text-lg font-bold tracking-tight text-white">
          Digi Guru
        </span>
      </Link>

      <nav className="hidden items-center gap-8 lg:flex">
        {LINKS.map((link) => (
          <a
            key={link.href}
            href={link.href}
            className="text-[16px] font-medium text-dg-gray-300 transition hover:text-white"
          >
            {link.label}
          </a>
        ))}
      </nav>

      <div className="hidden items-center gap-3 lg:flex">
        <Link
          href="/login"
          className="inline-flex min-h-11 items-center justify-center rounded-full border-[1.5px] border-lime-400 px-6 py-2.5 text-sm font-semibold text-lime-400 transition hover:bg-lime-400 hover:text-ink-950 focus-visible:outline-2 focus-visible:outline-indigo-300 focus-visible:outline-offset-2"
        >
          Log in
        </Link>
        <Link
          href="/signup"
          className="inline-flex min-h-11 items-center justify-center rounded-full bg-lime-400 px-6 py-2.5 text-sm font-semibold text-ink-950 transition hover:bg-lime-300 focus-visible:outline-2 focus-visible:outline-indigo-300 focus-visible:outline-offset-2"
        >
          Get Started
        </Link>
      </div>

      {/* Mobile: hamburger — min 44x44 touch target per DESIGN.md §7.1 */}
      <button
        type="button"
        aria-label={open ? "Close menu" : "Open menu"}
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
        className="flex h-11 w-11 items-center justify-center rounded-full text-white lg:hidden focus-visible:outline-2 focus-visible:outline-indigo-300 focus-visible:outline-offset-2"
      >
        <svg viewBox="0 0 24 24" width="24" height="24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
          {open ? (
            <path d="M6 6l12 12M18 6L6 18" />
          ) : (
            <path d="M4 7h16M4 12h16M4 17h16" />
          )}
        </svg>
      </button>

      {open && (
        <div className="fixed inset-0 top-0 z-10 flex flex-col bg-ink-950/96 pt-24 lg:hidden">
          <nav className="flex flex-col items-center gap-8 px-6">
            {LINKS.map((link) => (
              <a
                key={link.href}
                href={link.href}
                onClick={() => setOpen(false)}
                className="text-xl font-medium text-white"
              >
                {link.label}
              </a>
            ))}
          </nav>
          <div className="mt-auto flex flex-col gap-3 px-6 pb-10">
            <Link
              href="/login"
              onClick={() => setOpen(false)}
              className="inline-flex min-h-12 items-center justify-center rounded-full border-[1.5px] border-lime-400 px-6 py-3 text-base font-semibold text-lime-400"
            >
              Log in
            </Link>
            <Link
              href="/signup"
              onClick={() => setOpen(false)}
              className="inline-flex min-h-12 items-center justify-center rounded-full bg-lime-400 px-6 py-3 text-base font-semibold text-ink-950"
            >
              Get Started
            </Link>
          </div>
        </div>
      )}
    </header>
  );
}

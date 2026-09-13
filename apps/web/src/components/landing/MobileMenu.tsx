"use client";

import Link from "next/link";
import { useState } from "react";
import { IconClose, IconMenu } from "@/components/icons";

const links = [
  { href: "#how-it-works", label: "How it works" },
  { href: "#why", label: "Why it's different" },
  { href: "#cta", label: "For educators" },
];

// Only the interactive part of Nav is a client component — the rest of the
// page stays server-rendered. Per code-style.md: "use client" only where hooks
// genuinely require it.
export function MobileMenu() {
  const [open, setOpen] = useState(false);

  return (
    <div className="sm:hidden">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open}
        aria-label={open ? "Close menu" : "Open menu"}
        className="flex h-10 w-10 items-center justify-center rounded-full text-lavender-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-300"
      >
        {open ? <IconClose className="h-5 w-5" /> : <IconMenu className="h-5 w-5" />}
      </button>

      {open && (
        <div className="absolute inset-x-0 top-full border-t border-ink-800 bg-ink-950 px-6 py-6">
          <nav className="flex flex-col gap-4">
            {links.map((link) => (
              <a
                key={link.href}
                href={link.href}
                onClick={() => setOpen(false)}
                className="text-base text-gray-300 hover:text-lavender-50"
              >
                {link.label}
              </a>
            ))}
            <Link
              href="/login"
              onClick={() => setOpen(false)}
              className="text-base text-gray-300 hover:text-lavender-50"
            >
              Sign in
            </Link>
            <Link
              href="/signup"
              onClick={() => setOpen(false)}
              className="mt-2 inline-flex w-fit items-center rounded-full border-[1.5px] border-lime-400 px-5 py-2 text-sm font-semibold text-lime-400"
            >
              Register
            </Link>
          </nav>
        </div>
      )}
    </div>
  );
}

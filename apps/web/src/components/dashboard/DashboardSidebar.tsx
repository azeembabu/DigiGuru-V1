"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { useEffect, useRef, useState, type ReactNode } from "react";

import { IconBoard, IconBook, IconClose, IconMenu, IconShield, IconTarget } from "@/components/icons";
import { Logo } from "@/components/ui/Logo";

type NavItem = {
  href: string;
  label: string;
  icon: (props: { className?: string }) => ReactNode;
  /** True for `/dashboard#…` links, which share a page with Overview. */
  anchor?: boolean;
};

// Only routes that exist are listed. The reference console shows Analytics,
// Messages, Support and an e-commerce group; none of those have a gateway
// endpoint or a page behind them, and a nav row that goes nowhere costs the
// student more trust than the empty space costs the layout.
const sections: { label: string; items: NavItem[] }[] = [
  {
    label: "Dashboard",
    items: [
      { href: "/dashboard", label: "Overview", icon: IconTarget },
      { href: "/classroom", label: "Classroom", icon: IconBoard },
      { href: "/dashboard#enrolment", label: "Enrolment", icon: IconBook, anchor: true },
    ],
  },
  {
    label: "Settings",
    items: [{ href: "/dashboard#security", label: "Security", icon: IconShield, anchor: true }],
  },
];

const rowBase =
  "group relative flex items-center gap-3 rounded-[12px] px-3 py-2.5 text-[15px] transition-colors " +
  "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-300 " +
  "focus-visible:ring-offset-2 focus-visible:ring-offset-art-base";

function NavRow({ item, pathname, onNavigate }: { item: NavItem; pathname: string; onNavigate?: () => void }) {
  // Exactly one row may read as active. Route match wins outright, so Overview
  // owns `/dashboard`; the anchor rows underneath it target sections of that
  // same page, and lighting them at the same time would claim the student is in
  // three places at once. They instead get a dimmer "you are on this page"
  // marker while `/dashboard` is open, and never the full active treatment.
  const active = !item.anchor && pathname === item.href;
  const inSection = item.anchor === true && pathname === "/dashboard";
  const Icon = item.icon;

  return (
    <Link
      href={item.href}
      onClick={onNavigate}
      aria-current={active ? "page" : undefined}
      className={`${rowBase} ${
        active
          ? "bg-art-mid/70 font-semibold text-white"
          : "text-gray-300 hover:bg-art-mid/30 hover:text-white"
      }`}
    >
      {/* The green bar, not the fill alone, carries the state — §8 forbids
          colour-only signalling, and the bar survives forced-colours mode. */}
      <span
        aria-hidden="true"
        className={`absolute left-0 top-1/2 h-5 w-[3px] -translate-y-1/2 rounded-full bg-art-glow transition-opacity ${
          active ? "opacity-100" : inSection ? "opacity-40" : "opacity-0"
        }`}
      />
      <Icon className={`h-5 w-5 shrink-0 ${active ? "text-art-glow" : "text-gray-300 group-hover:text-art-edge"}`} />
      {item.label}
    </Link>
  );
}

function RailContent({ pathname, onNavigate }: { pathname: string; onNavigate?: () => void }) {
  return (
    <>
      <Link
        href="/dashboard"
        onClick={onNavigate}
        className="inline-flex rounded-[12px] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-300 focus-visible:ring-offset-2 focus-visible:ring-offset-art-base"
      >
        <Logo size="sm" />
      </Link>

      <nav className="mt-8 flex flex-col gap-7">
        {sections.map((section) => (
          <div key={section.label}>
            <p className="px-3 text-[12px] font-semibold uppercase leading-[1.4] tracking-[0.08em] text-gray-300/50">
              {section.label}
            </p>
            <div className="mt-2 flex flex-col gap-1">
              {section.items.map((item) => (
                <NavRow key={item.href} item={item} pathname={pathname} onNavigate={onNavigate} />
              ))}
            </div>
          </div>
        ))}
      </nav>
    </>
  );
}

/**
 * The dashboard's left rail. It owns its own `lg:hidden` trigger and drawer, so
 * the header stays a plain title-plus-sign-out bar and neither component has to
 * hold the other's open state.
 */
export function DashboardSidebar() {
  const pathname = usePathname();
  const [open, setOpen] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);

  function close() {
    setOpen(false);
    // Returning focus to the trigger keeps a keyboard user anchored where they
    // were; without it focus falls back to <body> and the next Tab restarts at
    // the top of the document.
    triggerRef.current?.focus();
  }

  useEffect(() => {
    if (!open) return;

    function onKeyDown(event: KeyboardEvent) {
      if (event.key !== "Escape") return;
      setOpen(false);
      triggerRef.current?.focus();
    }

    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [open]);

  return (
    <>
      <aside className="fixed inset-y-0 left-0 z-40 hidden w-60 flex-col overflow-y-auto border-r border-art-mid/60 bg-art-deep/40 px-4 py-6 lg:flex">
        <RailContent pathname={pathname} />
      </aside>

      <button
        ref={triggerRef}
        type="button"
        onClick={() => setOpen(true)}
        aria-expanded={open}
        aria-label="Open navigation"
        className="fixed left-4 top-3.5 z-40 flex h-10 w-10 items-center justify-center rounded-full border border-art-mid/60 bg-art-deep/80 text-white backdrop-blur focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-300 focus-visible:ring-offset-2 focus-visible:ring-offset-art-base lg:hidden"
      >
        <IconMenu className="h-5 w-5" />
      </button>

      {open && (
        <div className="fixed inset-0 z-50 lg:hidden">
          <div
            className="absolute inset-0 bg-art-base/80 backdrop-blur-sm"
            onClick={close}
            aria-hidden="true"
          />
          <div
            role="dialog"
            aria-modal="true"
            aria-label="Navigation"
            className="relative flex h-full w-60 max-w-[80%] flex-col overflow-y-auto border-r border-art-mid/60 bg-art-deep px-4 py-6"
          >
            <button
              type="button"
              onClick={close}
              aria-label="Close navigation"
              // Focused on mount so Escape and Tab both have a sensible anchor
              // inside the drawer the moment it opens.
              autoFocus
              className="absolute right-3 top-4 flex h-9 w-9 items-center justify-center rounded-full text-gray-300 transition-colors hover:text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-300 focus-visible:ring-offset-2 focus-visible:ring-offset-art-deep"
            >
              <IconClose className="h-5 w-5" />
            </button>
            <RailContent pathname={pathname} onNavigate={close} />
          </div>
        </div>
      )}
    </>
  );
}

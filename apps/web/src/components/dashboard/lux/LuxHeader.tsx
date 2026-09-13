"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useRef, useState } from "react";

import { IconLogout } from "@/components/icons";
import { logout } from "@/lib/student-context";
import type { AttentionItem } from "@/lib/dashboard-metrics";
import { Badge, luxButton } from "./shell";

// The top bar: the page's single `h1`, the notification bell, and the primary CTA.
//
// The bell is NOT decorative. There is no notifications endpoint in this product,
// so rather than render a bell that does nothing (or worse, a fake "3"), it
// reports items derived from state the payloads already carry: a reached quota,
// flashcards due, unattempted assessments, unmarked attempts. An empty list says
// so in words.

function Bell({ items }: { items: AttentionItem[] }) {
  const [open, setOpen] = useState(false);
  const wrapRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!open) return;

    function onKeyDown(event: KeyboardEvent) {
      if (event.key !== "Escape") return;
      setOpen(false);
      triggerRef.current?.focus();
    }
    // A click anywhere outside closes it. Pointerdown rather than click so the
    // panel does not survive until mouseup on a drag.
    function onPointerDown(event: PointerEvent) {
      if (event.target instanceof Node && wrapRef.current?.contains(event.target) === true) return;
      setOpen(false);
    }

    document.addEventListener("keydown", onKeyDown);
    document.addEventListener("pointerdown", onPointerDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      document.removeEventListener("pointerdown", onPointerDown);
    };
  }, [open]);

  const count = items.length;

  return (
    <div ref={wrapRef} className="relative">
      <button
        ref={triggerRef}
        type="button"
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open}
        aria-label={
          count === 0
            ? "Notifications: nothing needs your attention"
            : `Notifications: ${count} item${count === 1 ? "" : "s"} need your attention`
        }
        className="relative flex h-10 w-10 items-center justify-center rounded-full border border-lux-cream-300 bg-white text-lux-ink-700 transition-colors hover:bg-lux-cream-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#9c7a24] focus-visible:ring-offset-2 focus-visible:ring-offset-lux-cream-100"
      >
        <svg viewBox="0 0 20 20" aria-hidden="true" className="h-5 w-5 fill-none stroke-current stroke-[1.6]">
          <path
            d="M5.5 8.5a4.5 4.5 0 0 1 9 0c0 3 1 4.2 1.5 4.8H4c.5-.6 1.5-1.8 1.5-4.8Z"
            strokeLinejoin="round"
          />
          <path d="M8.2 16a1.9 1.9 0 0 0 3.6 0" strokeLinecap="round" />
        </svg>
        {count > 0 ? (
          // The count is text inside the dot, not the dot alone — a bare coloured
          // dot conveys "something" by colour only.
          <span className="absolute -right-0.5 -top-0.5 flex h-[18px] min-w-[18px] items-center justify-center rounded-full bg-lux-char-900 px-1 text-[10.5px] font-bold leading-none text-lux-mist-100">
            {count}
          </span>
        ) : null}
      </button>

      {open ? (
        <div
          role="region"
          aria-label="Notifications"
          className="absolute right-0 top-12 z-50 w-[min(21rem,calc(100vw-2rem))] rounded-[14px] border border-lux-cream-300 bg-white p-3 shadow-[0_12px_40px_-12px_rgba(28,27,24,0.3)]"
        >
          <p className="px-1 pb-2 text-[11px] font-semibold uppercase tracking-[0.1em] text-lux-ink-600">
            Needs your attention
          </p>
          {count === 0 ? (
            <p className="px-1 pb-1 text-[13.5px] leading-[1.6] text-lux-ink-600">
              Nothing needs your attention right now. Your revision queue is clear and every
              assessment has been attempted.
            </p>
          ) : (
            <ul className="flex flex-col gap-1">
              {items.map((item) => (
                <li key={item.id}>
                  <Link
                    href={item.href}
                    onClick={() => setOpen(false)}
                    className="flex items-start gap-2 rounded-[10px] px-2 py-2 text-[13.5px] leading-[1.5] text-lux-ink-900 hover:bg-lux-cream-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#9c7a24]"
                  >
                    <span className="mt-0.5 shrink-0">
                      <Badge tone={item.tone === "warn" ? "warn" : "neutral"}>
                        {item.tone === "warn" ? "Paused" : "To do"}
                      </Badge>
                    </span>
                    <span className="min-w-0">{item.text}</span>
                  </Link>
                </li>
              ))}
            </ul>
          )}
        </div>
      ) : null}
    </div>
  );
}

export function LuxHeader({
  items,
  cta,
}: {
  items: AttentionItem[];
  /** Resolved by the caller from the real resume + quota state. */
  cta: { href: string; label: string; hint: string | null };
}) {
  const router = useRouter();
  const [pending, setPending] = useState(false);

  async function onSignOut() {
    if (pending) return;
    setPending(true);
    try {
      await logout();
    } catch {
      // Deliberately swallowed, as in the previous header: the gateway clears
      // its own cookies, and a blip on the way back must not strand a student on
      // a dashboard they asked to leave.
    }
    router.replace("/login");
    router.refresh();
  }

  return (
    <header className="sticky top-0 z-30 border-b border-lux-cream-300 bg-lux-cream-100/90 backdrop-blur">
      <div className="mx-auto flex w-full max-w-[1200px] flex-wrap items-center justify-between gap-3 py-3 pl-16 pr-4 sm:pr-6 lg:pl-8">
        <div className="min-w-0">
          {/* The page's one and only h1. Everything else on the page is an h2 or
              lower, so a screen-reader outline has a single title. */}
          <h1 className="font-display text-[22px] font-bold leading-[1.25] text-lux-ink-900">
            Dashboard
          </h1>
          {cta.hint !== null ? (
            <p className="truncate text-[13px] leading-[1.45] text-lux-ink-600">{cta.hint}</p>
          ) : null}
        </div>

        <div className="flex items-center gap-2">
          <Bell items={items} />

          <button
            type="button"
            onClick={onSignOut}
            disabled={pending}
            aria-busy={pending}
            className="flex h-10 w-10 items-center justify-center rounded-full border border-lux-cream-300 bg-white text-lux-ink-700 transition-colors hover:bg-lux-cream-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#9c7a24] focus-visible:ring-offset-2 focus-visible:ring-offset-lux-cream-100 disabled:opacity-60"
            aria-label={pending ? "Signing out" : "Sign out"}
          >
            <IconLogout className="h-[18px] w-[18px]" />
          </button>

          <Link href={cta.href} className={luxButton("primary")}>
            {cta.label}
          </Link>
        </div>
      </div>
    </header>
  );
}

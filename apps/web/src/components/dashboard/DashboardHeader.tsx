"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";

import { IconLogout } from "@/components/icons";
import { logout } from "@/lib/student-context";

// The left rail (`DashboardSidebar`) owns the brand lockup now, so this bar
// deliberately carries no logo — repeating it beside the rail would give the
// page two competing anchors. On small screens the rail collapses to its own
// hamburger, which sits in the gutter this header reserves at `pl-14`.
//
// No search field, bell, or theme switch from the reference console: none of
// them has a gateway behind it, and a control that looks live but does nothing
// is a worse answer than an absent one.
const signOutButton =
  "inline-flex items-center gap-2 rounded-full border-[1.5px] border-art-mid bg-art-deep/70 " +
  "px-5 py-2.5 font-sans text-[15px] font-semibold text-white transition-colors " +
  "hover:border-art-edge hover:bg-art-mid/50 focus-visible:outline-none focus-visible:ring-2 " +
  "focus-visible:ring-indigo-300 focus-visible:ring-offset-2 focus-visible:ring-offset-art-base " +
  "disabled:cursor-not-allowed disabled:opacity-60";

export function DashboardHeader() {
  const router = useRouter();
  const [pending, setPending] = useState(false);

  async function onSignOut() {
    if (pending) return;
    setPending(true);

    try {
      await logout();
    } catch {
      // Deliberately swallowed. The gateway clears the auth cookies on its own
      // side, and a network blip on the way back must not strand the student on
      // a dashboard they have asked to leave — navigating away is the safer
      // failure. Any still-valid refresh row is reaped server-side on expiry.
    }

    router.replace("/login");
    // The dashboard's Server Components cached the authenticated render; without
    // this the back/forward cache can show it again after the cookies are gone.
    router.refresh();
  }

  return (
    <header className="sticky top-0 z-30 border-b border-art-mid/60 bg-art-base/80 backdrop-blur">
      <div className="mx-auto flex w-full max-w-6xl items-center justify-between gap-4 py-3.5 pl-14 pr-5 sm:pr-8 lg:pl-8">
        <div className="min-w-0">
          <p className="text-[13px] leading-[1.4] text-gray-300/60">Dashboard /</p>
          {/* Not an <h1>: the page's heading is the greeting in the content
              column below. This is a breadcrumb label, and two competing h1s
              would leave a screen-reader user with no single page title. */}
          <p className="truncate font-display text-[20px] font-bold leading-[1.3] text-white">
            Overview
          </p>
        </div>

        <button
          type="button"
          onClick={onSignOut}
          disabled={pending}
          aria-busy={pending}
          className={signOutButton}
        >
          <IconLogout className="h-5 w-5" />
          {pending ? "Signing out…" : "Sign out"}
        </button>
      </div>
    </header>
  );
}

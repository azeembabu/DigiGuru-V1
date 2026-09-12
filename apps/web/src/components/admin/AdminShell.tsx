"use client";

import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from "react";

import { AdminButton } from "@/components/admin/controls";
import { Banner, StatusPill, panelClass } from "@/components/admin/primitives";
import { Logo } from "@/components/ui/Logo";
import { ApiError } from "@/lib/api";
import { isAdmin, loadMe, type Me } from "@/lib/me";
import { logout } from "@/lib/student-context";

// The admin console shell: identity, navigation, and the gate in front of
// both.
//
// The gate here is a **UX** gate, not a security boundary. It exists so an
// admin sees the right navigation and a student sees an honest "not for you"
// instead of a screen full of 403s. Every byte of admin data is still
// protected by the gateway's own per-handler capability checks
// (`.claude/rules/security.md`) — nothing below is trusted to keep anyone out.

type AdminSession = {
  me: Me;
  /** Re-read `/me` after a change that can alter scopes. */
  refresh: () => Promise<void>;
};

const AdminSessionContext = createContext<AdminSession | null>(null);

/** Read the signed-in admin. Only valid inside `AdminShell`. */
export function useAdminSession(): AdminSession {
  const session = useContext(AdminSessionContext);
  if (!session) {
    throw new Error("useAdminSession must be used inside AdminShell");
  }
  return session;
}

type NavItem = {
  href: string;
  label: string;
  /** Omitted means every admin role sees it. */
  superAdminOnly?: boolean;
};

// Ordered by how often an admin reaches for them, with the academic hierarchy
// (Program > Semester > Course > Block) entered through Programs — that
// drill-down is the hierarchy, so it does not get four sidebar entries.
const NAV: NavItem[] = [
  { href: "/admin", label: "Overview" },
  { href: "/admin/students", label: "Students" },
  { href: "/admin/programs", label: "Programs" },
  { href: "/admin/lscs", label: "Learner Support Centres" },
  { href: "/admin/users", label: "Admin users", superAdminOnly: true },
];

export function AdminShell({ children }: { children: ReactNode }) {
  const router = useRouter();
  const [me, setMe] = useState<Me | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [denied, setDenied] = useState(false);

  const load = useCallback(async () => {
    try {
      const next = await loadMe();
      if (!isAdmin(next.role)) {
        setDenied(true);
        return;
      }
      setMe(next);
      setError(null);
    } catch (caught) {
      if (caught instanceof ApiError && caught.status === 401) {
        // Not signed in at all — send them to log in, and come back here.
        router.replace("/login?next=/admin");
        return;
      }
      setError(
        caught instanceof ApiError
          ? caught.message
          : "Could not load your account. Please try again.",
      );
    }
  }, [router]);

  useEffect(() => {
    // Wrapped rather than called bare: every `setState` in `load` happens
    // after an await, so this is a subscription to an external system (the
    // gateway), not a synchronous cascade out of render.
    void (async () => {
      await load();
    })();
  }, [load]);

  if (denied) {
    return (
      <AdminFrame>
        <div className={`${panelClass} mx-auto mt-16 max-w-md px-6 py-8 text-center`}>
          <h1 className="font-display text-xl font-semibold text-gray-900">
            This area is for administrators
          </h1>
          <p className="mt-2 text-sm text-gray-500">
            Your account does not manage programs or students. Your classroom and course
            progress are on your dashboard.
          </p>
          <div className="mt-6 flex justify-center">
            <AdminButton type="button" onClick={() => router.replace("/dashboard")}>
              Go to my dashboard
            </AdminButton>
          </div>
        </div>
      </AdminFrame>
    );
  }

  if (error) {
    return (
      <AdminFrame>
        <div className="mx-auto mt-16 max-w-md space-y-4">
          <Banner tone="danger">{error}</Banner>
          <AdminButton type="button" onClick={() => void load()}>
            Try again
          </AdminButton>
        </div>
      </AdminFrame>
    );
  }

  if (!me) {
    return (
      <AdminFrame>
        <p className="mt-16 text-center text-sm text-gray-500" role="status">
          Loading your console…
        </p>
      </AdminFrame>
    );
  }

  return (
    <AdminSessionContext.Provider value={{ me, refresh: load }}>
      <AdminFrame>
        <div className="flex min-h-dvh flex-col lg:flex-row">
          <Sidebar me={me} />
          <main className="min-w-0 flex-1 px-4 py-6 sm:px-6 lg:px-8">{children}</main>
        </div>
      </AdminFrame>
    </AdminSessionContext.Provider>
  );
}

/**
 * The light ground. The root `<body>` is `ink-950` for the marketing site and
 * the classroom; the console overrides it here rather than in the root layout
 * so the dark surfaces are untouched.
 */
function AdminFrame({ children }: { children: ReactNode }) {
  return <div className="min-h-dvh bg-lavender-50 text-gray-900">{children}</div>;
}

function Sidebar({ me }: { me: Me }) {
  const pathname = usePathname();
  const router = useRouter();
  const [signingOut, setSigningOut] = useState(false);

  const items = NAV.filter((item) => !item.superAdminOnly || me.role === "super_admin");

  async function onSignOut() {
    setSigningOut(true);
    try {
      await logout();
    } catch {
      // A failed logout call still means the admin wants out of this browser.
      // The cookie is httpOnly, so the only thing the client can do is leave —
      // the gateway's own session expiry is the backstop.
    }
    router.replace("/login");
  }

  return (
    <nav
      aria-label="Admin"
      className="shrink-0 border-b border-lavender-200 bg-white px-4 py-4 lg:w-64 lg:border-b-0 lg:border-r lg:px-5 lg:py-6"
    >
      <Link
        href="/admin"
        className="inline-flex rounded-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-400 focus-visible:ring-offset-2 focus-visible:ring-offset-white"
      >
        <Logo size="sm" tone="light" />
      </Link>

      <ul className="mt-5 flex flex-wrap gap-1 lg:mt-7 lg:flex-col lg:flex-nowrap">
        {items.map((item) => {
          // `/admin` would otherwise light up on every child route.
          const active =
            item.href === "/admin" ? pathname === "/admin" : pathname.startsWith(item.href);

          return (
            <li key={item.href}>
              <Link
                href={item.href}
                aria-current={active ? "page" : undefined}
                className={`block rounded-sm px-3 py-2 text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-400 focus-visible:ring-offset-2 focus-visible:ring-offset-white ${
                  active
                    ? "bg-lime-500/15 text-gray-900"
                    : "text-gray-500 hover:bg-lavender-50 hover:text-gray-900"
                }`}
              >
                {item.label}
              </Link>
            </li>
          );
        })}
      </ul>

      <div className="mt-6 border-t border-lavender-200 pt-4 lg:mt-10">
        <p className="truncate text-sm font-medium text-gray-900" title={me.email}>
          {me.full_name ?? me.email}
        </p>
        <p className="mt-1 truncate text-xs text-gray-500">{me.email}</p>

        <div className="mt-2">
          <StatusPill tone={me.role === "super_admin" ? "info" : "neutral"}>
            {me.role === "super_admin" ? "Super admin" : "Sub-admin"}
          </StatusPill>
        </div>

        {/* A sub-admin can only ever touch these programs — showing the scope
            up front is what stops a 403 later reading as a bug. */}
        {me.role === "sub_admin" ? (
          <div className="mt-3">
            <p className="text-xs font-medium uppercase tracking-wide text-gray-500">
              Scoped to
            </p>
            {me.scopes.length === 0 ? (
              <p className="mt-1 text-xs text-danger">
                No programs assigned — ask a super admin to grant you a scope.
              </p>
            ) : (
              <ul className="mt-1 space-y-0.5">
                {me.scopes.map((scope) => (
                  <li key={scope.program_id} className="truncate text-xs text-gray-900">
                    {scope.name}
                  </li>
                ))}
              </ul>
            )}
          </div>
        ) : null}

        <div className="mt-4">
          <AdminButton
            type="button"
            variant="outline-light"
            onClick={() => void onSignOut()}
            pending={signingOut}
            pendingLabel="Signing out…"
            className="w-full"
          >
            Sign out
          </AdminButton>
        </div>
      </div>
    </nav>
  );
}

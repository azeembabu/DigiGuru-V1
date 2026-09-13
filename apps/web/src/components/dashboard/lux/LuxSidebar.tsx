"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { useEffect, useId, useRef, useState, type ReactNode } from "react";

import { IconBoard, IconBook, IconClose, IconMenu, IconShield, IconTarget } from "@/components/icons";
import { Logo } from "@/components/ui/Logo";

// The dark charcoal rail. It is the only dark surface in the page's chrome, and
// every text step on it was picked against `lux-char-900`: `lux-mist-100` for
// active rows, `lux-mist-300` for resting rows, `lux-mist-400` for the section
// labels and the user card's secondary line — all ≥4.5:1, so nothing on the rail
// is decoration-only text.

export type CourseOption = { id: string; code: string; name: string };

type NavItem = {
  label: string;
  icon: (props: { className?: string }) => ReactNode;
  /** A route, or an anchor into the dashboard page itself. */
  href?: string;
  /** Set instead of `href` for a destination that does not exist yet. */
  pending?: boolean;
};

// Only these four routes exist. The brief's "Support" and "Settings" rows have
// no page and no endpoint behind them, so they render as visibly disabled
// `aria-disabled` rows rather than as links that 404 — a nav row that looks live
// and goes nowhere costs a student more trust than an honest "soon" does.
const NAV: { label: string; items: NavItem[] }[] = [
  {
    label: "Study",
    items: [
      { label: "Dashboard", href: "/dashboard", icon: IconTarget },
      { label: "Classroom", href: "/classroom", icon: IconBoard },
      { label: "Assessments", href: "/exams", icon: IconBook },
      { label: "Performance", href: "/dashboard#performance", icon: IconShield },
    ],
  },
  {
    label: "Account",
    items: [
      { label: "Switch module", href: "/portal", icon: IconTarget },
      { label: "Support", pending: true, icon: IconBook },
      { label: "Settings", pending: true, icon: IconShield },
    ],
  },
];

const rowBase =
  "group relative flex items-center gap-3 rounded-[11px] px-3 py-2.5 text-[14.5px] transition-colors " +
  "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#b8851f] focus-visible:ring-offset-2 " +
  "focus-visible:ring-offset-lux-char-950";

function NavRow({
  item,
  pathname,
  onNavigate,
}: {
  item: NavItem;
  pathname: string;
  onNavigate?: () => void;
}) {
  const Icon = item.icon;
  // An anchor row never takes the active treatment: it targets a section of the
  // page the Dashboard row already owns, and lighting both would claim the
  // student is in two places.
  const isAnchor = item.href?.includes("#") === true;
  const active = !isAnchor && item.href !== undefined && pathname === item.href;

  const marker = (
    // The gold bar, not the fill alone, carries the state — it survives
    // forced-colours mode and a monochrome screen, where a tinted background
    // does not.
    <span
      aria-hidden="true"
      className={`absolute left-0 top-1/2 h-5 w-[3px] -translate-y-1/2 rounded-full bg-[#b8851f] transition-opacity ${
        active ? "opacity-100" : "opacity-0"
      }`}
    />
  );

  if (item.pending === true) {
    return (
      <span
        aria-disabled="true"
        className={`${rowBase} cursor-not-allowed text-lux-mist-400/70`}
        // Conveyed in text, not only by the dimming — "soon" is the reason, and
        // a greyed row with no explanation reads as a bug.
      >
        <Icon className="h-[18px] w-[18px] shrink-0 text-lux-mist-400/60" />
        <span className="flex-1">{item.label}</span>
        <span className="rounded-full border border-lux-char-600 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-[0.06em] text-lux-mist-400">
          Soon
        </span>
      </span>
    );
  }

  return (
    <Link
      href={item.href ?? "/dashboard"}
      onClick={onNavigate}
      aria-current={active ? "page" : undefined}
      className={`${rowBase} ${
        active
          ? "bg-lux-char-800 font-semibold text-lux-mist-100"
          : "text-lux-mist-300 hover:bg-lux-char-800/60 hover:text-lux-mist-100"
      }`}
    >
      {marker}
      <Icon
        className={`h-[18px] w-[18px] shrink-0 ${active ? "text-[#ddb463]" : "text-lux-mist-400 group-hover:text-lux-mist-100"}`}
      />
      {item.label}
    </Link>
  );
}

/**
 * The search field.
 *
 * There is no student search endpoint, so this does not pretend to be one: it
 * filters the course table and the weak-topic panel already on screen, which is
 * real work done on real rows. The "/" hint is wired to the same behaviour the
 * hint claims.
 */
function SearchField({
  value,
  onChange,
  inputRef,
}: {
  value: string;
  onChange: (next: string) => void;
  inputRef: React.RefObject<HTMLInputElement | null>;
}) {
  const id = useId();

  return (
    <div className="mt-6">
      <label htmlFor={id} className="sr-only">
        Filter your courses and topics
      </label>
      <div className="relative">
        <svg
          viewBox="0 0 16 16"
          aria-hidden="true"
          className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 fill-none stroke-lux-mist-400 stroke-[1.6]"
        >
          <circle cx="7" cy="7" r="4.5" />
          <path d="M10.5 10.5 14 14" strokeLinecap="round" />
        </svg>
        <input
          id={id}
          ref={inputRef}
          type="search"
          value={value}
          onChange={(event) => onChange(event.target.value)}
          placeholder="Filter courses, topics"
          className="w-full rounded-[11px] border border-lux-char-700 bg-lux-char-900 py-2 pl-9 pr-12 text-[14px] text-lux-mist-100 placeholder:text-lux-mist-400 focus-visible:border-[#b8851f] focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[#b8851f]"
        />
        <kbd
          aria-hidden="true"
          className="pointer-events-none absolute right-2.5 top-1/2 -translate-y-1/2 rounded-[6px] border border-lux-char-600 bg-lux-char-800 px-1.5 py-0.5 font-sans text-[11px] font-semibold text-lux-mist-400"
        >
          /
        </kbd>
      </div>
    </div>
  );
}

/**
 * The workspace selector.
 *
 * It lists the courses the student actually has assessments in, plus "All
 * courses", and changing it filters the performance chart and the table. With a
 * single enrolled course it still renders — disabled, naming that course — rather
 * than disappearing, because the rail's job includes telling the student which
 * workspace they are in.
 */
function CourseSelect({
  courses,
  value,
  onChange,
}: {
  courses: CourseOption[];
  value: string;
  onChange: (next: string) => void;
}) {
  const id = useId();
  const only = courses.length === 1 ? courses[0] : null;

  return (
    <div className="mt-6">
      <label
        htmlFor={id}
        className="text-[11px] font-semibold uppercase tracking-[0.1em] text-lux-mist-400"
      >
        Workspace
      </label>
      <select
        id={id}
        value={value}
        disabled={courses.length <= 1}
        onChange={(event) => onChange(event.target.value)}
        className="mt-1.5 w-full rounded-[11px] border border-lux-char-700 bg-lux-char-900 px-3 py-2 text-[14px] font-semibold text-lux-mist-100 focus-visible:border-[#b8851f] focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[#b8851f] disabled:cursor-default disabled:text-lux-mist-300"
      >
        {courses.length === 0 ? (
          <option value="all">No course yet</option>
        ) : (
          <>
            {courses.length > 1 ? <option value="all">All courses</option> : null}
            {courses.map((course) => (
              <option key={course.id} value={course.id}>
                {course.code} · {course.name}
              </option>
            ))}
          </>
        )}
      </select>
      {only !== null ? (
        <p className="mt-1.5 text-[12px] leading-[1.5] text-lux-mist-400">
          Your only enrolled course this semester.
        </p>
      ) : null}
    </div>
  );
}

/** The bottom user card: avatar, name, roll number. */
function UserCard({
  name,
  rollNumber,
  programme,
}: {
  name: string | null;
  rollNumber: string | null;
  programme: string | null;
}) {
  // Initials rather than a fetched image: `avatar_url` is null for every seeded
  // student and the payload makes no promise it will not be, so the fallback is
  // the normal case and has to look deliberate.
  const initials =
    name === null
      ? "—"
      : name
          .split(/\s+/)
          .filter((part) => part.length > 0)
          .slice(0, 2)
          .map((part) => part[0]?.toUpperCase() ?? "")
          .join("");

  return (
    <div className="mt-6 flex items-center gap-3 rounded-[13px] border border-lux-char-700 bg-lux-char-900 p-3">
      <span
        aria-hidden="true"
        className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-lux-char-700 font-display text-[13px] font-bold text-[#ddb463]"
      >
        {initials}
      </span>
      <div className="min-w-0">
        {/* A name the gateway has not sent yet is a dash, not a guess. */}
        <p className="truncate text-[14px] font-semibold leading-[1.35] text-lux-mist-100">
          {name ?? "—"}
        </p>
        <p className="truncate text-[12px] leading-[1.4] text-lux-mist-400">
          {rollNumber ?? "—"}
          {programme === null ? "" : ` · ${programme}`}
        </p>
      </div>
    </div>
  );
}

export type SidebarProps = {
  courses: CourseOption[];
  courseFilter: string;
  onCourseFilter: (next: string) => void;
  search: string;
  onSearch: (next: string) => void;
  name: string | null;
  rollNumber: string | null;
  programme: string | null;
};

function RailBody({
  props,
  pathname,
  onNavigate,
  searchRef,
}: {
  props: SidebarProps;
  pathname: string;
  onNavigate?: () => void;
  searchRef: React.RefObject<HTMLInputElement | null>;
}) {
  return (
    <>
      <Link
        href="/dashboard"
        onClick={onNavigate}
        className="inline-flex rounded-[11px] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#b8851f] focus-visible:ring-offset-2 focus-visible:ring-offset-lux-char-950"
      >
        <Logo size="sm" />
      </Link>

      <CourseSelect
        courses={props.courses}
        value={props.courseFilter}
        onChange={props.onCourseFilter}
      />
      <SearchField value={props.search} onChange={props.onSearch} inputRef={searchRef} />

      <nav aria-label="Student sections" className="mt-7 flex flex-1 flex-col gap-6">
        {NAV.map((section) => (
          <div key={section.label}>
            <p className="px-3 text-[11px] font-semibold uppercase leading-[1.4] tracking-[0.1em] text-lux-mist-400">
              {section.label}
            </p>
            <div className="mt-1.5 flex flex-col gap-0.5">
              {section.items.map((item) => (
                <NavRow
                  key={item.label}
                  item={item}
                  pathname={pathname}
                  onNavigate={item.pending === true ? undefined : onNavigate}
                />
              ))}
            </div>
          </div>
        ))}
      </nav>

      <UserCard name={props.name} rollNumber={props.rollNumber} programme={props.programme} />
    </>
  );
}

export function LuxSidebar(props: SidebarProps) {
  const pathname = usePathname();
  const [open, setOpen] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const desktopSearchRef = useRef<HTMLInputElement>(null);
  const drawerSearchRef = useRef<HTMLInputElement>(null);

  // The "/" hint has to be true. Ignored while the student is already typing in
  // a field, so pressing "/" inside the search box (or any future input) types a
  // slash instead of stealing focus from under them.
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.key !== "/" || event.metaKey || event.ctrlKey || event.altKey) return;
      const target = event.target;
      if (
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        target instanceof HTMLSelectElement ||
        (target instanceof HTMLElement && target.isContentEditable)
      ) {
        return;
      }
      const field = desktopSearchRef.current ?? drawerSearchRef.current;
      if (field === null) return;
      event.preventDefault();
      field.focus();
    }

    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, []);

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

  function close() {
    setOpen(false);
    triggerRef.current?.focus();
  }

  return (
    <>
      <aside className="fixed inset-y-0 left-0 z-40 hidden w-[264px] flex-col overflow-y-auto bg-lux-char-950 px-4 py-6 lg:flex">
        <RailBody props={props} pathname={pathname} searchRef={desktopSearchRef} />
      </aside>

      <button
        ref={triggerRef}
        type="button"
        onClick={() => setOpen(true)}
        aria-expanded={open}
        aria-label="Open navigation"
        className="fixed left-4 top-3 z-40 flex h-10 w-10 items-center justify-center rounded-full border border-lux-cream-300 bg-white text-lux-ink-900 shadow-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#9c7a24] focus-visible:ring-offset-2 focus-visible:ring-offset-lux-cream-100 lg:hidden"
      >
        <IconMenu className="h-5 w-5" />
      </button>

      {open ? (
        <div className="fixed inset-0 z-50 lg:hidden">
          <div
            className="absolute inset-0 bg-lux-char-950/70 backdrop-blur-sm"
            onClick={close}
            aria-hidden="true"
          />
          <div
            role="dialog"
            aria-modal="true"
            aria-label="Navigation"
            className="relative flex h-full w-[264px] max-w-[85%] flex-col overflow-y-auto bg-lux-char-950 px-4 py-6"
          >
            <button
              type="button"
              onClick={close}
              aria-label="Close navigation"
              autoFocus
              className="absolute right-3 top-4 flex h-9 w-9 items-center justify-center rounded-full text-lux-mist-300 transition-colors hover:text-lux-mist-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#b8851f] focus-visible:ring-offset-2 focus-visible:ring-offset-lux-char-950"
            >
              <IconClose className="h-5 w-5" />
            </button>
            <RailBody
              props={props}
              pathname={pathname}
              onNavigate={close}
              searchRef={drawerSearchRef}
            />
          </div>
        </div>
      ) : null}
    </>
  );
}

// Single-weight line icons, 1.5–2px stroke, rounded caps — per DESIGN.md §4.
// viewBox is always 24x24 so icons drop in at any size via className.

type IconProps = { className?: string };

const base = {
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.75,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
};

export function IconBoard({ className }: IconProps) {
  return (
    <svg {...base} className={className} aria-hidden="true">
      <rect x="3" y="4.5" width="18" height="12" rx="2" />
      <path d="M8 20h8M12 16.5V20" />
      <path d="M6.5 9h6M6.5 12h4" />
    </svg>
  );
}

export function IconBook({ className }: IconProps) {
  return (
    <svg {...base} className={className} aria-hidden="true">
      <path d="M4 5.5C4 4.67 4.67 4 5.5 4H12v16H5.5A1.5 1.5 0 0 1 4 18.5v-13Z" />
      <path d="M20 5.5c0-.83-.67-1.5-1.5-1.5H12v16h6.5c.83 0 1.5-.67 1.5-1.5v-13Z" />
    </svg>
  );
}

export function IconClock({ className }: IconProps) {
  return (
    <svg {...base} className={className} aria-hidden="true">
      <circle cx="12" cy="12" r="8.25" />
      <path d="M12 7.5V12l3 2" />
    </svg>
  );
}

export function IconPin({ className }: IconProps) {
  return (
    <svg {...base} className={className} aria-hidden="true">
      <path d="M12 21s-6.5-5.35-6.5-10.5a6.5 6.5 0 0 1 13 0C18.5 15.65 12 21 12 21Z" />
      <circle cx="12" cy="10.5" r="2.25" />
    </svg>
  );
}

export function IconSparkle({ className }: IconProps) {
  return (
    <svg {...base} className={className} aria-hidden="true">
      <path d="M12 3.5c.6 2.9 1.4 4.6 2.4 5.6 1 1 2.7 1.8 5.6 2.4-2.9.6-4.6 1.4-5.6 2.4-1 1-1.8 2.7-2.4 5.6-.6-2.9-1.4-4.6-2.4-5.6-1-1-2.7-1.8-5.6-2.4 2.9-.6 4.6-1.4 5.6-2.4 1-1 1.8-2.7 2.4-5.6Z" />
    </svg>
  );
}

export function IconArrowUpRight({ className }: IconProps) {
  return (
    <svg {...base} className={className} aria-hidden="true">
      <path d="M7 17 17 7M9 7h8v8" />
    </svg>
  );
}

export function IconMenu({ className }: IconProps) {
  return (
    <svg {...base} className={className} aria-hidden="true">
      <path d="M4 7h16M4 12h16M4 17h16" />
    </svg>
  );
}

export function IconClose({ className }: IconProps) {
  return (
    <svg {...base} className={className} aria-hidden="true">
      <path d="M6 6l12 12M18 6 6 18" />
    </svg>
  );
}

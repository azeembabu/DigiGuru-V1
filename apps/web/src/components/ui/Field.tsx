import type { ReactNode } from "react";

// Shared label + error scaffolding for every auth input, so the error wiring
// (aria-invalid, aria-describedby, the icon-plus-text pairing DESIGN.md §8
// requires instead of colour alone) is written once rather than per field.

export function Field({
  id,
  label,
  error,
  hint,
  children,
}: {
  id: string;
  label: string;
  error?: string;
  hint?: string;
  children: ReactNode;
}) {
  return (
    <div>
      <label htmlFor={id} className="block text-sm font-medium text-lavender-50">
        {label}
      </label>
      <div className="mt-1">{children}</div>
      {error ? (
        <p id={`${id}-error`} role="alert" className="mt-1 flex items-start gap-1.5 text-xs text-danger">
          <span aria-hidden="true">&#9888;</span>
          <span>{error}</span>
        </p>
      ) : hint ? (
        <p id={`${id}-hint`} className="mt-1 text-xs text-gray-300">
          {hint}
        </p>
      ) : null}
    </div>
  );
}

/**
 * The shared control styling — same treatment for input and select.
 *
 * Translucent rather than a flat fill, so the auth route's green surface shows
 * through and the form reads as part of the artwork instead of a grey box
 * pasted over it. The focus ring stays `indigo-300` per DESIGN.md §5.1/§8:
 * it is the one state that must never be re-themed, because a focus indicator
 * tinted into the surrounding green would lose the contrast that makes it
 * usable.
 */
export const controlClass =
  "w-full rounded-sm border bg-white/[0.04] px-3.5 py-2 text-[15px] text-lavender-50 " +
  "backdrop-blur-sm placeholder:text-gray-500 transition-colors focus-visible:outline-none " +
  "focus-visible:ring-2 focus-visible:ring-indigo-300 focus-visible:ring-offset-2 " +
  "focus-visible:ring-offset-art-base";

export function controlBorder(hasError: boolean): string {
  return hasError
    ? "border-danger"
    : "border-lime-300/20 hover:border-lime-300/40 focus-visible:border-lime-300/50";
}

"use client";

import { useEffect, useId, useRef, useState } from "react";

import { controlBorder, controlClass } from "@/components/ui/Field";

// A custom listbox rather than a native `<select>`.
//
// The native control cannot do what this design needs: browsers do not let you
// style an `<option>`'s hover state, and on Windows the popup is drawn by the
// OS, so "light black text that turns black on hover" is simply unreachable
// there. This renders the list ourselves and keeps the accessibility contract
// the native element would have given us for free:
//
// - `role="combobox"` trigger with `aria-expanded` / `aria-controls`, and
//   `role="listbox"` / `role="option"` with `aria-selected` in the panel.
// - Full keyboard support: Up/Down, Home/End, Enter/Space to choose, Escape to
//   dismiss, Tab to leave. `aria-activedescendant` tracks the highlighted row
//   so a screen reader announces it while focus stays on the trigger.
// - Closing always returns focus to the trigger, so keyboard users are never
//   dropped back at the top of the document.
// - A hidden input carries the value under the field's `name`, so the form
//   still reads it through `FormData` exactly as it did with a `<select>`.
//
// The panel is deliberately light (white, with dark text) against the dark
// page: it reads as a surface lifted above the form rather than a hole cut
// into it, and it is where the "light black -> black on hover" text treatment
// lives.

type Option = { id: string; name: string };

export function SelectInput({
  id,
  options,
  placeholder,
  value,
  onValueChange,
  disabled = false,
  hasError = false,
  hasHint = false,
  className = "",
}: {
  id: string;
  options: Option[];
  placeholder: string;
  /** Controlled value. Omit to let the component track its own. */
  value?: string;
  onValueChange?: (value: string) => void;
  disabled?: boolean;
  hasError?: boolean;
  hasHint?: boolean;
  className?: string;
}) {
  const listId = useId();
  const wrapRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const optionRefs = useRef<(HTMLLIElement | null)[]>([]);

  const [internal, setInternal] = useState("");
  const selected = value !== undefined ? value : internal;

  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(-1);

  const selectedOption = options.find((o) => o.id === selected) ?? null;
  const isEmpty = options.length === 0;

  function commit(next: string) {
    if (value === undefined) setInternal(next);
    onValueChange?.(next);
  }

  function close(refocus = true) {
    setOpen(false);
    setActive(-1);
    if (refocus) triggerRef.current?.focus();
  }

  function openList() {
    if (disabled || isEmpty) return;
    setOpen(true);
    // Land on the current choice, or the first row when nothing is chosen.
    setActive(Math.max(0, options.findIndex((o) => o.id === selected)));
  }

  // Dismiss on an outside pointer press. Focus is not restored here: the user
  // is already reaching for something else.
  useEffect(() => {
    if (!open) return;
    function onPointerDown(event: PointerEvent) {
      if (!wrapRef.current?.contains(event.target as Node)) close(false);
    }
    document.addEventListener("pointerdown", onPointerDown);
    return () => document.removeEventListener("pointerdown", onPointerDown);
  }, [open]);

  // Keep the highlighted row inside the scroll box.
  useEffect(() => {
    if (open && active >= 0) optionRefs.current[active]?.scrollIntoView({ block: "nearest" });
  }, [open, active]);

  function onKeyDown(event: React.KeyboardEvent) {
    if (disabled || isEmpty) return;

    if (!open) {
      if (["ArrowDown", "ArrowUp", "Enter", " "].includes(event.key)) {
        event.preventDefault();
        openList();
      }
      return;
    }

    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        setActive((i) => (i + 1) % options.length);
        break;
      case "ArrowUp":
        event.preventDefault();
        setActive((i) => (i - 1 + options.length) % options.length);
        break;
      case "Home":
        event.preventDefault();
        setActive(0);
        break;
      case "End":
        event.preventDefault();
        setActive(options.length - 1);
        break;
      case "Enter":
      case " ":
        event.preventDefault();
        if (active >= 0) {
          commit(options[active].id);
          close();
        }
        break;
      case "Escape":
        event.preventDefault();
        close();
        break;
      case "Tab":
        close(false);
        break;
    }
  }

  return (
    <div ref={wrapRef} className="relative">
      {/* The form reads the value from here, so nothing downstream changes. */}
      <input type="hidden" name={id} value={selected} />

      <button
        ref={triggerRef}
        type="button"
        id={id}
        role="combobox"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? listId : undefined}
        aria-activedescendant={open && active >= 0 ? `${listId}-${active}` : undefined}
        aria-invalid={hasError || undefined}
        aria-describedby={hasError ? `${id}-error` : hasHint ? `${id}-hint` : undefined}
        disabled={disabled || isEmpty}
        onClick={() => (open ? close() : openList())}
        onKeyDown={onKeyDown}
        className={`${controlClass} ${controlBorder(hasError)} flex items-center justify-between gap-2 text-left disabled:cursor-not-allowed disabled:opacity-60 ${className}`}
      >
        <span className={selectedOption ? "text-lavender-50" : "text-gray-500"}>
          {selectedOption ? selectedOption.name : placeholder}
        </span>
        <svg
          aria-hidden="true"
          viewBox="0 0 20 20"
          className={`h-4 w-4 shrink-0 text-lime-300 transition-transform duration-150 ${open ? "rotate-180" : ""}`}
          fill="none"
          stroke="currentColor"
          strokeWidth="1.75"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          <path d="m5 7.5 5 5 5-5" />
        </svg>
      </button>

      {open ? (
        <ul
          id={listId}
          role="listbox"
          aria-label={placeholder}
          className="absolute z-50 mt-2 max-h-60 w-full overflow-y-auto rounded-md border border-lavender-200 bg-white p-1.5 shadow-[0_16px_40px_-8px_rgba(5,9,12,0.65)]"
        >
          {options.map((option, i) => {
            const isSelected = option.id === selected;
            const isActive = i === active;
            return (
              <li
                key={option.id}
                id={`${listId}-${i}`}
                ref={(el) => {
                  optionRefs.current[i] = el;
                }}
                role="option"
                aria-selected={isSelected}
                onPointerEnter={() => setActive(i)}
                onClick={() => {
                  commit(option.id);
                  close();
                }}
                className={[
                  "flex cursor-pointer items-center justify-between gap-2 rounded-sm px-3 py-2 text-[15px]",
                  "transition-colors duration-100",
                  // Resting text is a softened black; hover/keyboard focus
                  // deepens it to full black and lifts the row.
                  isActive ? "bg-lavender-100 text-gray-900" : "text-gray-900/65",
                  isSelected ? "font-semibold" : "font-normal",
                ].join(" ")}
              >
                <span>{option.name}</span>
                {isSelected ? (
                  <svg
                    aria-hidden="true"
                    viewBox="0 0 20 20"
                    className="h-4 w-4 shrink-0 text-lime-500"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  >
                    <path d="m4.5 10.5 4 4 7-8" />
                  </svg>
                ) : null}
              </li>
            );
          })}
        </ul>
      ) : null}
    </div>
  );
}

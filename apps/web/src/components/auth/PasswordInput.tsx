"use client";

import { useState } from "react";

import { controlBorder, controlClass } from "@/components/ui/Field";

export function PasswordInput({
  id,
  autoComplete,
  hasError = false,
  hasHint = false,
}: {
  id: string;
  autoComplete: "current-password" | "new-password";
  hasError?: boolean;
  hasHint?: boolean;
}) {
  const [visible, setVisible] = useState(false);

  return (
    <div className="relative">
      <input
        id={id}
        name={id}
        type={visible ? "text" : "password"}
        autoComplete={autoComplete}
        required
        aria-invalid={hasError || undefined}
        aria-describedby={hasError ? `${id}-error` : hasHint ? `${id}-hint` : undefined}
        className={`${controlClass} ${controlBorder(hasError)} pr-16`}
      />
      <button
        type="button"
        onClick={() => setVisible((v) => !v)}
        // The control is a real button with a text label rather than an icon,
        // so its state is announced and its tap target clears 44px.
        aria-pressed={visible}
        className="absolute inset-y-0 right-0 flex items-center rounded-sm px-3 text-xs font-semibold text-indigo-300 transition-colors hover:text-lavender-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-300"
      >
        {visible ? "Hide" : "Show"}
      </button>
    </div>
  );
}

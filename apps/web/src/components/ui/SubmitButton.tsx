import { buttonClass, type Variant } from "@/components/ui/Button";

export function SubmitButton({
  children,
  pending = false,
  pendingLabel = "Working\u2026",
  variant = "primary",
  className = "",
}: {
  children: string;
  pending?: boolean;
  pendingLabel?: string;
  variant?: Variant;
  className?: string;
}) {
  return (
    <button
      type="submit"
      disabled={pending}
      // `aria-busy` rather than swapping the accessible name silently: a
      // screen reader announces the state change instead of a new button.
      aria-busy={pending || undefined}
      className={buttonClass(variant, `w-full disabled:cursor-not-allowed disabled:opacity-60 ${className}`)}
    >
      {pending ? pendingLabel : children}
    </button>
  );
}

// Form-level failure banner (bad credentials, server unreachable, 409s).
// Uses `danger` — never lime — per DESIGN.md §8, and pairs the colour with an
// icon and text so colour is not the only signal.

export function FormError({ message }: { message: string | null }) {
  if (!message) return null;

  return (
    <div
      role="alert"
      className="flex items-start gap-2.5 rounded-sm border border-danger/40 bg-danger/10 px-3.5 py-3 text-sm text-lavender-50"
    >
      <span aria-hidden="true" className="text-danger">
        &#9888;
      </span>
      <span>{message}</span>
    </div>
  );
}

// Ring + wordmark, per DESIGN.md §6 — lime ring only, never indigo in the mark itself.
export function Logo({ className = "" }: { className?: string }) {
  return (
    <span className={`inline-flex items-center gap-2 font-display font-bold text-lg ${className}`}>
      <span
        aria-hidden="true"
        className="h-5 w-5 rounded-full border-2 border-lime-400"
      />
      Digi Guru
    </span>
  );
}

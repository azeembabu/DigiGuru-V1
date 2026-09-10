import { Lock } from 'lucide-react';

interface ReadOnlyFieldProps {
  label: string;
  value: string | null | undefined;
  /** Shown muted when there is no value. */
  fallback?: string;
  /** Explains why the field cannot be changed (only for a protected field). */
  hint?: string;
}

/**
 * A value the student cannot edit. Rendered as static text rather than a
 * disabled input so it never looks like an editable control that is merely
 * switched off — and carries a small lock so the reason is obvious.
 */
export function ReadOnlyField({ label, value, fallback = '—', hint }: ReadOnlyFieldProps) {
  return (
    <div>
      <div className="flex items-center gap-1.5">
        <dt className="text-xs font-medium uppercase tracking-wide text-ink-500">{label}</dt>
        <Lock size={11} className="text-ink-400" aria-hidden />
        <span className="sr-only">(read-only)</span>
      </div>
      <dd className="mt-1.5 rounded-lg border border-slate-200 bg-slate-50 px-3.5 py-2.5 text-sm text-ink-700">
        {value || fallback}
      </dd>
      {hint && <p className="mt-1.5 text-xs leading-relaxed text-ink-400">{hint}</p>}
    </div>
  );
}

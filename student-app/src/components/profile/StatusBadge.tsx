import type { AccountStatus } from '../../types/profile';

const STYLES: Record<AccountStatus | 'DEFAULT', string> = {
  ACTIVE: 'bg-green-50 text-green-700 ring-green-600/20',
  INACTIVE: 'bg-slate-100 text-ink-500 ring-slate-400/20',
  SUSPENDED: 'bg-red-50 text-red-700 ring-red-600/20',
  DEFAULT: 'bg-slate-100 text-ink-500 ring-slate-400/20',
};

/**
 * Account status pill. Colour is paired with the written label, so status is
 * never communicated by colour alone.
 */
export function StatusBadge({ status }: { status: AccountStatus }) {
  const label = status.charAt(0) + status.slice(1).toLowerCase();
  return (
    <span
      className={[
        'inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-xs font-medium ring-1 ring-inset',
        STYLES[status] ?? STYLES.DEFAULT,
      ].join(' ')}
    >
      <span className="h-1.5 w-1.5 rounded-full bg-current" aria-hidden />
      {label}
    </span>
  );
}

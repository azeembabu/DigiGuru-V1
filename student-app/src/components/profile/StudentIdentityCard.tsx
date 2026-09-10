import { GraduationCap, Hash } from 'lucide-react';
import { StatusBadge } from './StatusBadge';
import { formatAccountStatus, initialsFromName } from '../../lib/format';
import type { StudentProfile } from '../../types/profile';

/**
 * Student identity at a glance: avatar, full name, Roll Number and status,
 * plus the headline academic placement. Every value comes from the
 * authenticated profile response — nothing here is hard-coded.
 */
export function StudentIdentityCard({ profile }: { profile: StudentProfile }) {
  const { fullName, rollNumber, program, semester, accountStatus } = profile;

  return (
    <section className="overflow-hidden rounded-xl border border-slate-200 bg-white shadow-sm">
      <div className="flex flex-col items-center px-6 pb-6 pt-8 text-center">
        <span
          className="flex h-20 w-20 items-center justify-center rounded-full bg-dg-900 text-2xl font-semibold text-white"
          aria-hidden
        >
          {initialsFromName(fullName)}
        </span>

        <h2 className="mt-4 text-lg font-semibold tracking-tight text-ink-900">{fullName || '—'}</h2>

        <div className="mt-1.5 flex items-center gap-1.5 text-sm text-ink-500">
          <Hash size={13} aria-hidden />
          <span className="font-medium tabular-nums">{rollNumber || '—'}</span>
        </div>

        <div className="mt-3">
          <StatusBadge status={accountStatus} />
        </div>
      </div>

      <dl className="grid grid-cols-1 gap-px border-t border-slate-100 bg-slate-100 text-sm">
        <div className="flex items-center justify-between bg-white px-6 py-3">
          <dt className="text-ink-500">Program</dt>
          <dd className="text-right font-medium text-ink-900">{program?.name ?? '—'}</dd>
        </div>
        <div className="flex items-center justify-between bg-white px-6 py-3">
          <dt className="text-ink-500">Semester</dt>
          <dd className="flex items-center gap-1.5 text-right font-medium text-ink-900">
            <GraduationCap size={14} className="text-ink-400" aria-hidden />
            {semester?.name ?? '—'}
          </dd>
        </div>
        <div className="flex items-center justify-between bg-white px-6 py-3">
          <dt className="text-ink-500">Account status</dt>
          <dd className="text-right font-medium text-ink-900">{formatAccountStatus(accountStatus)}</dd>
        </div>
      </dl>
    </section>
  );
}

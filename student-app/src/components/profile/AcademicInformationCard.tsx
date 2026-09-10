import { Building2, GraduationCap, Landmark } from 'lucide-react';
import { SectionCard } from './SectionCard';
import type { StudentProfile } from '../../types/profile';

/**
 * Academic placement. Program, Semester and LSC are institution-controlled —
 * every value is read-only here and there is no control to change them.
 */
export function AcademicInformationCard({ profile }: { profile: StudentProfile }) {
  const { program, semester, lsc } = profile;

  const rows = [
    {
      label: 'Program',
      value: program ? `${program.name}${program.code ? ` (${program.code})` : ''}` : '—',
      icon: GraduationCap,
    },
    {
      label: 'Current Semester',
      value: semester ? `Semester ${semester.semesterNumber} — ${semester.name}` : '—',
      icon: Building2,
    },
    {
      label: 'LSC',
      value: lsc ? `${lsc.name}${lsc.code ? ` (${lsc.code})` : ''}` : '—',
      icon: Landmark,
    },
  ] as const;

  return (
    <SectionCard
      title="Academic Information"
      description="Set by your institution at admission."
    >
      <dl className="divide-y divide-slate-100">
        {rows.map(({ label, value, icon: Icon }) => (
          <div key={label} className="flex items-start justify-between gap-4 py-3.5 first:pt-0 last:pb-0">
            <dt className="flex items-center gap-2.5 text-sm text-ink-500">
              <Icon size={16} className="shrink-0 text-ink-400" aria-hidden />
              {label}
            </dt>
            <dd className="text-right text-sm font-medium text-ink-900">{value}</dd>
          </div>
        ))}
      </dl>

      <div className="mt-4 rounded-lg border border-dashed border-slate-200 bg-slate-50/70 px-4 py-3">
        <p className="text-xs font-medium text-ink-700">These details are read-only</p>
        <p className="mt-1 text-xs leading-relaxed text-ink-500">
          Program, Semester and LSC are set by your institution. Contact your Learner Support Centre to
          request a correction.
        </p>
      </div>
    </SectionCard>
  );
}

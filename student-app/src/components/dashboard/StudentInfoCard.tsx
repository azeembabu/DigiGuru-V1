import { Link } from 'react-router-dom';
import { ArrowRight } from 'lucide-react';

/**
 * Student identity card (spec §14) — small, minimal, no sensitive info.
 */

interface Props {
  fullName: string;
  rollNumber: string;
  program: string | null | undefined;
  semester: string | null | undefined;
}

export function StudentInfoCard({ fullName, rollNumber, program, semester }: Props) {
  return (
    <div className="rounded-xl border border-slate-200 bg-white p-6 shadow-sm">
      <p className="text-xs font-semibold uppercase tracking-wider text-ink-500">Student</p>
      <p className="mt-3 text-lg font-semibold text-ink-900">{fullName}</p>
      <dl className="mt-3 space-y-1.5 text-sm text-ink-500">
        <div>Roll No. {rollNumber}</div>
        {program && <div>{program}</div>}
        {semester && <div>{semester}</div>}
      </dl>
      <Link
        to="/student/profile"
        className="mt-4 inline-flex items-center gap-1.5 text-sm font-semibold text-dg-700 hover:text-dg-900"
      >
        View Profile <ArrowRight size={14} aria-hidden />
      </Link>
    </div>
  );
}

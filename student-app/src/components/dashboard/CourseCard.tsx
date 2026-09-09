import { Link } from 'react-router-dom';
import { ArrowRight } from 'lucide-react';

/**
 * Course card (spec §10): name, code, progress, open action.
 */

interface Props {
  course: {
    id: string;
    name: string;
    code: string;
  };
  progressPercent: number;
}

export function CourseCard({ course, progressPercent }: Props) {
  return (
    <div className="flex flex-col rounded-xl border border-slate-200 bg-white p-5 shadow-sm">
      <div className="flex items-start justify-between gap-3">
        <h3 className="text-base font-semibold leading-snug text-ink-900">{course.name}</h3>
        <span className="shrink-0 rounded-md bg-slate-100 px-2 py-0.5 text-[11px] font-medium text-ink-500">
          {course.code}
        </span>
      </div>

      <div className="mt-4">
        <div className="flex items-center justify-between text-xs font-medium text-ink-700">
          <span>{progressPercent > 0 ? `${progressPercent}% complete` : 'Not started'}</span>
          <span>{progressPercent}%</span>
        </div>
        <div className="mt-1.5 h-1.5 overflow-hidden rounded-full bg-slate-100">
          <div className="h-full rounded-full bg-dg-600" style={{ width: `${Math.min(progressPercent, 100)}%` }} />
        </div>
      </div>

      <Link
        to="/student/class"
        className="mt-4 inline-flex items-center gap-1.5 text-sm font-semibold text-dg-700 hover:text-dg-900"
      >
        Open Course <ArrowRight size={14} aria-hidden />
      </Link>
    </div>
  );
}

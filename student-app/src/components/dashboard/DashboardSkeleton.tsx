/**
 * Dashboard skeleton (spec §23): neutral placeholders mirroring the page layout.
 */

export function DashboardSkeleton() {
  return (
    <div className="mx-auto w-full max-w-6xl px-4 py-8 sm:px-6 lg:px-8" aria-busy="true" aria-live="polite">
      <span className="sr-only">Loading your dashboard…</span>

      {/* Greeting */}
      <div className="animate-pulse space-y-3">
        <div className="h-8 w-64 rounded-lg bg-slate-200" />
        <div className="h-4 w-80 rounded bg-slate-100" />
      </div>

      {/* Continue Learning + Student info */}
      <div className="mt-8 grid gap-6 lg:grid-cols-3">
        <div className="animate-pulse rounded-xl border border-slate-200 bg-white p-6 shadow-sm lg:col-span-2">
          <div className="h-3 w-32 rounded bg-slate-100" />
          <div className="mt-4 h-6 w-56 rounded bg-slate-200" />
          <div className="mt-2 h-4 w-40 rounded bg-slate-100" />
          <div className="mt-6 h-2 w-full rounded-full bg-slate-100" />
          <div className="mt-6 h-12 w-44 rounded-lg bg-slate-200" />
        </div>
        <div className="animate-pulse rounded-xl border border-slate-200 bg-white p-6 shadow-sm">
          <div className="h-3 w-16 rounded bg-slate-100" />
          <div className="mt-4 h-5 w-40 rounded bg-slate-200" />
          <div className="mt-4 space-y-2">
            <div className="h-4 w-full rounded bg-slate-100" />
            <div className="h-4 w-3/4 rounded bg-slate-100" />
            <div className="h-4 w-2/3 rounded bg-slate-100" />
          </div>
        </div>
      </div>

      {/* Courses */}
      <div className="mt-6 animate-pulse rounded-xl border border-slate-200 bg-white p-6 shadow-sm">
        <div className="h-5 w-32 rounded bg-slate-200" />
        <div className="mt-4 grid gap-4 sm:grid-cols-2">
          <div className="h-36 rounded-lg bg-slate-100" />
          <div className="h-36 rounded-lg bg-slate-100" />
        </div>
      </div>

      {/* Activity + Quick Access */}
      <div className="mt-6 grid gap-6 lg:grid-cols-3">
        <div className="animate-pulse rounded-xl border border-slate-200 bg-white p-6 shadow-sm lg:col-span-2">
          <div className="h-5 w-36 rounded bg-slate-200" />
          <div className="mt-4 space-y-4">
            <div className="h-10 w-full rounded bg-slate-100" />
            <div className="h-10 w-5/6 rounded bg-slate-100" />
            <div className="h-10 w-4/6 rounded bg-slate-100" />
          </div>
        </div>
        <div className="animate-pulse rounded-xl border border-slate-200 bg-white p-6 shadow-sm">
          <div className="h-5 w-28 rounded bg-slate-200" />
          <div className="mt-4 grid grid-cols-2 gap-3">
            <div className="h-11 rounded-lg bg-slate-100" />
            <div className="h-11 rounded-lg bg-slate-100" />
            <div className="h-11 rounded-lg bg-slate-100" />
            <div className="h-11 rounded-lg bg-slate-100" />
          </div>
        </div>
      </div>
    </div>
  );
}

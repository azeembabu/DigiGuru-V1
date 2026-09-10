/**
 * Profile skeleton: neutral placeholders mirroring the real two-column layout,
 * so the page does not jump when data arrives.
 */
export function ProfileSkeleton() {
  return (
    <div className="mx-auto w-full max-w-6xl px-4 py-6 sm:px-6 lg:px-8" aria-busy="true" aria-live="polite">
      <span className="sr-only">Loading your profile…</span>

      {/* Header */}
      <div className="animate-pulse space-y-3">
        <div className="h-4 w-32 rounded bg-slate-100" />
        <div className="h-7 w-40 rounded bg-slate-200" />
        <div className="h-4 w-72 rounded bg-slate-100" />
      </div>

      <div className="mt-6 grid gap-6 lg:grid-cols-3">
        {/* Main column */}
        <div className="space-y-6 lg:col-span-2">
          {[0, 1, 2].map((i) => (
            <div key={i} className="animate-pulse rounded-xl border border-slate-200 bg-white shadow-sm">
              <div className="space-y-2 border-b border-slate-100 px-6 py-4">
                <div className="h-4 w-40 rounded bg-slate-200" />
                <div className="h-3 w-64 rounded bg-slate-100" />
              </div>
              <div className="space-y-4 px-6 py-5">
                <div className="h-11 w-full rounded-lg bg-slate-100" />
                <div className="h-11 w-full rounded-lg bg-slate-100" />
                <div className="h-11 w-2/3 rounded-lg bg-slate-100" />
              </div>
            </div>
          ))}
        </div>

        {/* Secondary column */}
        <div className="space-y-6">
          <div className="animate-pulse rounded-xl border border-slate-200 bg-white shadow-sm">
            <div className="flex flex-col items-center px-6 pb-6 pt-8">
              <div className="h-20 w-20 rounded-full bg-slate-200" />
              <div className="mt-4 h-5 w-36 rounded bg-slate-200" />
              <div className="mt-2 h-4 w-24 rounded bg-slate-100" />
            </div>
            <div className="space-y-3 border-t border-slate-100 px-6 py-4">
              <div className="h-4 w-full rounded bg-slate-100" />
              <div className="h-4 w-full rounded bg-slate-100" />
            </div>
          </div>

          <div className="animate-pulse rounded-xl border border-slate-200 bg-white p-6 shadow-sm">
            <div className="h-4 w-36 rounded bg-slate-200" />
            <div className="mt-4 space-y-3">
              <div className="h-4 w-full rounded bg-slate-100" />
              <div className="h-4 w-3/4 rounded bg-slate-100" />
              <div className="h-4 w-2/3 rounded bg-slate-100" />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

import * as React from 'react';

interface SectionCardProps {
  title: string;
  description?: string;
  /** Small adornment beside the title (e.g. a lock badge). */
  badge?: React.ReactNode;
  children: React.ReactNode;
  className?: string;
}

/**
 * One collapsible-free content section of the profile page: a heading with an
 * optional description, then the section body. Uses the shared Card look
 * (white, thin border, restrained shadow) so every section reads as one system.
 */
export function SectionCard({ title, description, badge, children, className = '' }: SectionCardProps) {
  return (
    <section className={['rounded-xl border border-slate-200 bg-white shadow-sm', className].join(' ')}>
      <header className="border-b border-slate-100 px-5 py-4 sm:px-6">
        <div className="flex flex-wrap items-center gap-2">
          <h2 className="text-base font-semibold tracking-tight text-ink-900">{title}</h2>
          {badge}
        </div>
        {description && <p className="mt-1 text-sm text-ink-500">{description}</p>}
      </header>
      <div className="px-5 py-5 sm:px-6">{children}</div>
    </section>
  );
}

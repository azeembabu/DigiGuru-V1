import * as React from 'react';

type Props = React.SelectHTMLAttributes<HTMLSelectElement> & {
  label?: string;
  error?: string;
  hint?: string;
  placeholder?: string;
  options: ReadonlyArray<string | { value: string; label: string }>;
};

export const Select = React.forwardRef<HTMLSelectElement, Props>(function Select(
  { label, error, hint, placeholder, options, id, className = '', children, ...props },
  ref,
) {
  const selectId = id ?? (label ? label.toLowerCase().replace(/\s+/g, '-') : undefined);
  const describedBy = error ? `${selectId}-error` : hint ? `${selectId}-hint` : undefined;

  return (
    <div className="space-y-1.5">
      {label && (
        <label htmlFor={selectId} className="block text-sm font-medium text-ink-900">
          {label}
          {props.required && <span className="ml-1 text-red-500" aria-hidden>*</span>}
        </label>
      )}
      <div className="relative">
        <select
          ref={ref}
          id={selectId}
          aria-invalid={!!error}
          aria-describedby={describedBy}
          className={[
            'block w-full appearance-none rounded-xl border bg-white px-3.5 py-2.5 pr-10 text-[15px] text-ink-900',
            'shadow-sm transition-colors duration-150',
            'focus:outline-none focus:ring-2 focus:ring-dg-500/20 focus:border-dg-500',
            error ? 'border-red-300 focus:border-red-400 focus:ring-red-500/20' : 'border-slate-200 hover:border-slate-300',
            'disabled:bg-slate-50 disabled:text-slate-500',
            className,
          ].join(' ')}
          {...props}
        >
          {placeholder && <option value="">{placeholder}</option>}
          {options.map((opt) => {
            const value = typeof opt === 'string' ? opt : opt.value;
            const optLabel = typeof opt === 'string' ? opt : opt.label;
            return (
              <option key={value} value={value}>
                {optLabel}
              </option>
            );
          })}
          {children}
        </select>
        <span className="pointer-events-none absolute inset-y-0 right-3 flex items-center text-ink-400" aria-hidden>
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
            <path d="M4 6l4 4 4-4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        </span>
      </div>
      {error && (
        <p id={`${selectId}-error`} className="text-sm text-red-600" role="alert">
          {error}
        </p>
      )}
      {!error && hint && (
        <p id={`${selectId}-hint`} className="text-sm text-ink-500">
          {hint}
        </p>
      )}
    </div>
  );
});

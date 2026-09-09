import * as React from 'react';

type Props = React.InputHTMLAttributes<HTMLInputElement> & {
  label?: string;
  error?: string;
  hint?: string;
};

export const Input = React.forwardRef<HTMLInputElement, Props>(function Input(
  { label, error, hint, id, className = '', ...props },
  ref,
) {
  const inputId = id ?? (label ? label.toLowerCase().replace(/\s+/g, '-') : undefined);
  const describedBy = error ? `${inputId}-error` : hint ? `${inputId}-hint` : undefined;

  return (
    <div className="space-y-1.5">
      {label && (
        <label htmlFor={inputId} className="block text-sm font-medium text-ink-900">
          {label}
          {props.required && <span className="ml-1 text-red-500" aria-hidden>*</span>}
        </label>
      )}
      <input
        ref={ref}
        id={inputId}
        aria-invalid={!!error}
        aria-describedby={describedBy}
        className={[
          'block w-full rounded-xl border bg-white px-3.5 py-2.5 text-[15px] text-ink-900 placeholder:text-ink-400',
          'shadow-sm transition-colors duration-150',
          'focus:outline-none focus:ring-2 focus:ring-dg-500/20 focus:border-dg-500',
          error ? 'border-red-300 focus:border-red-400 focus:ring-red-500/20' : 'border-slate-200 hover:border-slate-300',
          'disabled:bg-slate-50 disabled:text-slate-500',
          className,
        ].join(' ')}
        {...props}
      />
      {error && (
        <p id={`${inputId}-error`} className="text-sm text-red-600" role="alert">
          {error}
        </p>
      )}
      {!error && hint && (
        <p id={`${inputId}-hint`} className="text-sm text-ink-500">
          {hint}
        </p>
      )}
    </div>
  );
});

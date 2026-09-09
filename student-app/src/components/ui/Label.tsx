import * as React from 'react';

export function Label({
  className = '',
  ...props
}: React.LabelHTMLAttributes<HTMLLabelElement>) {
  return <label className={['text-sm font-medium text-ink-900', className].join(' ')} {...props} />;
}

export function FieldHint({ children, id }: { children: React.ReactNode; id?: string }) {
  return (
    <p id={id} className="text-sm text-ink-500">
      {children}
    </p>
  );
}

export function FieldError({ children, id }: { children: React.ReactNode; id?: string }) {
  return (
    <p id={id} className="text-sm text-red-600" role="alert">
      {children}
    </p>
  );
}

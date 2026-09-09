import * as React from 'react';

type BadgeVariant = 'default' | 'success' | 'warning' | 'danger' | 'neutral' | 'info';

const variantClasses: Record<BadgeVariant, string> = {
  default: 'bg-indigo-50 text-indigo-700 ring-indigo-200',
  success: 'bg-emerald-50 text-emerald-700 ring-emerald-200',
  warning: 'bg-amber-50 text-amber-700 ring-amber-200',
  danger: 'bg-red-50 text-red-700 ring-red-200',
  neutral: 'bg-admin-100 text-admin-700 ring-admin-200',
  info: 'bg-sky-50 text-sky-700 ring-sky-200',
};

export interface BadgeProps extends React.HTMLAttributes<HTMLSpanElement> {
  variant?: BadgeVariant;
}

export function Badge({ variant = 'default', className = '', children, ...props }: BadgeProps) {
  return (
    <span
      className={[
        'inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium ring-1 ring-inset',
        variantClasses[variant],
        className,
      ].join(' ')}
      {...props}
    >
      {children}
    </span>
  );
}

export function StatusBadge({ status }: { status: string }) {
  const normalized = status.toUpperCase();
  let variant: BadgeVariant = 'neutral';
  if (normalized === 'ACTIVE') variant = 'success';
  else if (normalized === 'INACTIVE') variant = 'neutral';
  else if (normalized === 'SUSPENDED') variant = 'danger';

  return <Badge variant={variant}>{status}</Badge>;
}

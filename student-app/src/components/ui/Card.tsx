import * as React from 'react';

type Props = React.HTMLAttributes<HTMLDivElement> & {
  variant?: 'glass' | 'solid' | 'subtle';
  padding?: 'none' | 'sm' | 'md' | 'lg';
};

const variantCls: Record<NonNullable<Props['variant']>, string> = {
  glass: 'glass-card rounded-[28px]',
  solid: 'bg-white border border-slate-200 rounded-2xl shadow-soft',
  subtle: 'bg-white/80 border border-slate-100 rounded-2xl',
};

const paddingCls: Record<NonNullable<Props['padding']>, string> = {
  none: '',
  sm: 'p-5',
  md: 'p-6 sm:p-8',
  lg: 'p-8 sm:p-10',
};

export function Card({ variant = 'glass', padding = 'md', className = '', children, ...props }: Props) {
  return (
    <div className={[variantCls[variant], paddingCls[padding], className].join(' ')} {...props}>
      {children}
    </div>
  );
}

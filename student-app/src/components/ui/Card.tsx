import * as React from 'react';

type Props = React.HTMLAttributes<HTMLDivElement> & {
  /** solid = white card on neutral bg (default). subtle = flat white. */
  variant?: 'solid' | 'subtle';
  padding?: 'none' | 'sm' | 'md' | 'lg';
};

/**
 * Card spec (§26): white cards on #F8FAFC background, radius 8–12px,
 * card padding 20–24px, minimal shadow. No glassmorphism.
 */
const variantCls: Record<NonNullable<Props['variant']>, string> = {
  solid: 'bg-white border border-slate-200 rounded-xl shadow-sm',
  subtle: 'bg-white border border-slate-100 rounded-xl',
};

const paddingCls: Record<NonNullable<Props['padding']>, string> = {
  none: '',
  sm: 'p-5', // 20px
  md: 'p-6', // 24px
  lg: 'p-6 sm:p-8',
};

export function Card({ variant = 'solid', padding = 'md', className = '', children, ...props }: Props) {
  return (
    <div className={[variantCls[variant], paddingCls[padding], className].join(' ')} {...props}>
      {children}
    </div>
  );
}

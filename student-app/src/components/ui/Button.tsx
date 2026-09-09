import * as React from 'react';

type Variant = 'primary' | 'secondary' | 'ghost';
type Size = 'sm' | 'md' | 'lg';

type Props = React.ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: Variant;
  size?: Size;
  loading?: boolean;
};

/**
 * Primary button spec (§9): 44–48px height, radius 8–10px, font-weight 600.
 * One consistent primary style — no giant pills.
 */
const variantCls: Record<Variant, string> = {
  primary:
    'bg-dg-900 text-white hover:bg-dg-950 active:bg-dg-950 ' +
    'shadow-sm hover:shadow disabled:bg-slate-300 disabled:shadow-none',
  secondary:
    'bg-white text-dg-900 border border-slate-200 hover:bg-slate-50 hover:border-slate-300 ' +
    'shadow-sm disabled:bg-slate-100 disabled:text-slate-400',
  ghost: 'bg-transparent text-ink-700 hover:bg-slate-100 hover:text-ink-900 disabled:text-slate-400',
};

const sizeCls: Record<Size, string> = {
  sm: 'h-9 px-4 text-sm',
  md: 'h-11 px-5 text-sm', // 44px
  lg: 'h-12 px-6 text-[15px]', // 48px
};

export function Button({
  variant = 'primary',
  size = 'md',
  loading,
  disabled,
  children,
  className = '',
  ...props
}: Props) {
  return (
    <button
      disabled={disabled || loading}
      className={[
        'inline-flex items-center justify-center gap-2 rounded-lg font-semibold',
        'transition-colors duration-150 focus:outline-none focus-visible:ring-2 focus-visible:ring-dg-600 focus-visible:ring-offset-2',
        'disabled:cursor-not-allowed',
        variantCls[variant],
        sizeCls[size],
        className,
      ].join(' ')}
      {...props}
    >
      {loading && (
        <span className="h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent" aria-hidden />
      )}
      {children}
    </button>
  );
}

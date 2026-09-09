import * as React from 'react';

type Variant = 'primary' | 'secondary' | 'ghost';
type Size = 'sm' | 'md' | 'lg';

type Props = React.ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: Variant;
  size?: Size;
  loading?: boolean;
};

const variantCls: Record<Variant, string> = {
  primary:
    'bg-dg-900 text-white hover:bg-dg-950 active:bg-black ' +
    'shadow-[0_4px_16px_rgba(14,75,58,0.28)] hover:shadow-[0_6px_24px_rgba(14,75,58,0.32)] ' +
    'disabled:bg-slate-300 disabled:shadow-none',
  secondary:
    'bg-white text-dg-900 border border-dg-100 hover:bg-dg-50 hover:border-dg-200 ' +
    'shadow-soft disabled:bg-slate-100 disabled:text-slate-400',
  ghost:
    'bg-transparent text-ink-700 hover:bg-white/70 hover:text-dg-900 ' +
    'disabled:text-slate-400',
};

const sizeCls: Record<Size, string> = {
  sm: 'h-9 px-4 text-sm',
  md: 'h-11 px-6 text-[15px]',
  lg: 'h-12 px-8 text-[15px]',
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
        'inline-flex items-center justify-center gap-2 rounded-full font-medium',
        'transition-all duration-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-dg-500 focus-visible:ring-offset-2',
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

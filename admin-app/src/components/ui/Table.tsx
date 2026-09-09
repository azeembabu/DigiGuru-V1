import * as React from 'react';

export function TableWrapper({
  children,
  className = '',
  ...props
}: React.HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={['overflow-hidden rounded-xl border border-admin-200 bg-white shadow-card', className].join(
        ' ',
      )}
      {...props}
    >
      <div className="overflow-x-auto admin-scrollbar">{children}</div>
    </div>
  );
}

export function Table({ className = '', children, ...props }: React.TableHTMLAttributes<HTMLTableElement>) {
  return (
    <table className={['w-full text-left text-sm', className].join(' ')} {...props}>
      {children}
    </table>
  );
}

export function TableHead({ className = '', children, ...props }: React.HTMLAttributes<HTMLTableSectionElement>) {
  return (
    <thead className={['bg-admin-50/80 text-xs font-medium uppercase tracking-wider text-admin-500', className].join(' ')} {...props}>
      {children}
    </thead>
  );
}

export function TableBody({ className = '', children, ...props }: React.HTMLAttributes<HTMLTableSectionElement>) {
  return (
    <tbody className={['divide-y divide-admin-100 bg-white', className].join(' ')} {...props}>
      {children}
    </tbody>
  );
}

export function TableRow({ className = '', children, ...props }: React.HTMLAttributes<HTMLTableRowElement>) {
  return (
    <tr className={['transition-colors hover:bg-admin-50/60', className].join(' ')} {...props}>
      {children}
    </tr>
  );
}

export function TableHeaderCell({
  className = '',
  children,
  ...props
}: React.ThHTMLAttributes<HTMLTableCellElement>) {
  return (
    <th className={['px-4 py-3 font-medium whitespace-nowrap', className].join(' ')} {...props}>
      {children}
    </th>
  );
}

export function TableCell({
  className = '',
  children,
  ...props
}: React.TdHTMLAttributes<HTMLTableCellElement>) {
  return (
    <td className={['px-4 py-3 text-admin-700', className].join(' ')} {...props}>
      {children}
    </td>
  );
}

export function EmptyRow({ colSpan, message }: { colSpan: number; message: string }) {
  return (
    <tr>
      <td colSpan={colSpan} className="px-4 py-12 text-center">
        <p className="text-sm text-admin-500">{message}</p>
      </td>
    </tr>
  );
}

export function TableSkeleton({ rows = 5, cols = 5 }: { rows?: number; cols?: number }) {
  return (
    <>
      {Array.from({ length: rows }).map((_, r) => (
        <tr key={r} className="animate-pulse">
          {Array.from({ length: cols }).map((__, c) => (
            <td key={c} className="px-4 py-4">
              <div className="h-3 rounded bg-admin-100" />
            </td>
          ))}
        </tr>
      ))}
    </>
  );
}

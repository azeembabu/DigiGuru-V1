/**
 * Shared date/duration formatting for dashboard components.
 */

/** "Today" / "Yesterday" / "Sep 9" label for a timestamp. */
export function formatDayLabel(iso: string): string {
  const d = new Date(iso);
  const now = new Date();
  const yesterday = new Date(now);
  yesterday.setDate(now.getDate() - 1);
  const isSameDay = (a: Date, b: Date) =>
    a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();
  if (isSameDay(d, now)) return 'Today';
  if (isSameDay(d, yesterday)) return 'Yesterday';
  return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
}

/** "5 min" for a duration; null when under a minute (nothing meaningful to show). */
export function formatDurationMinutes(seconds: number): string | null {
  if (seconds < 60) return null;
  return `${Math.round(seconds / 60)} min`;
}

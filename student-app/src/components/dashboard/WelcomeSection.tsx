/**
 * Welcome section (spec §7): time-based greeting + academic identity line.
 * Data comes from the authenticated student record — never hard-coded.
 */

interface Props {
  fullName: string;
  program: string | null | undefined;
  semester: string | null | undefined;
  rollNumber: string;
}

function greetingFor(d: Date): string {
  const h = d.getHours();
  if (h < 12) return 'Good morning';
  if (h < 17) return 'Good afternoon';
  return 'Good evening';
}

export function WelcomeSection({ fullName, program, semester, rollNumber }: Props) {
  const firstName = fullName.split(' ')[0] || fullName;
  const meta = [program, semester, rollNumber ? `Roll No. ${rollNumber}` : null]
    .filter(Boolean)
    .join(' · ');

  return (
    <section>
      <h1 className="text-[26px] font-semibold leading-tight tracking-tight text-ink-900 sm:text-3xl">
        {greetingFor(new Date())}, {firstName}
      </h1>
      {meta && <p className="mt-1.5 text-sm text-ink-500 sm:text-[15px]">{meta}</p>}
    </section>
  );
}

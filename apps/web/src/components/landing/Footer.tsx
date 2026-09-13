import { Logo } from "@/components/ui/Logo";

const columns = [
  {
    heading: "Product",
    links: [
      { href: "#how-it-works", label: "How it works" },
      { href: "#why", label: "Why it's different" },
    ],
  },
  {
    heading: "Company",
    links: [
      { href: "#top", label: "About" },
      { href: "#cta", label: "Contact" },
    ],
  },
];

export function Footer() {
  return (
    <footer className="border-t border-ink-800">
      <div className="mx-auto max-w-6xl px-6 py-14">
        <div className="flex flex-col gap-10 sm:flex-row sm:justify-between">
          <div className="max-w-xs">
            <Logo />
            <p className="mt-3 text-sm leading-relaxed text-gray-300">
              Live AI tutoring, on your syllabus.
            </p>
          </div>

          <div className="grid grid-cols-2 gap-10 sm:gap-16">
            {columns.map((col) => (
              <div key={col.heading}>
                <p className="text-xs font-semibold tracking-[0.08em] text-gray-300 uppercase">
                  {col.heading}
                </p>
                <ul className="mt-4 space-y-3">
                  {col.links.map((link) => (
                    <li key={link.label}>
                      <a href={link.href} className="text-sm text-gray-300 hover:text-lavender-50">
                        {link.label}
                      </a>
                    </li>
                  ))}
                </ul>
              </div>
            ))}
          </div>
        </div>

        <div className="mt-14 flex flex-col gap-2 border-t border-ink-800 pt-6 text-xs text-gray-500 sm:flex-row sm:items-center sm:justify-between">
          <p>&copy; {new Date().getFullYear()} Digi Guru. All rights reserved.</p>
          <p>Session length and content availability depend on your enrolled program.</p>
        </div>
      </div>
    </footer>
  );
}

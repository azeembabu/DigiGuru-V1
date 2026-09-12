import { Logo } from "@/components/ui/Logo";
import { MobileMenu } from "@/components/landing/MobileMenu";

const links = [
  { href: "#how-it-works", label: "How it works" },
  { href: "#why", label: "Why it's different" },
  { href: "#cta", label: "For educators" },
];

export function Nav() {
  return (
    <header className="sticky top-0 z-30 border-b border-ink-800 bg-ink-950/80 backdrop-blur-md">
      <div className="relative mx-auto flex max-w-6xl items-center justify-between px-6 py-4">
        <a href="#top" className="text-lavender-50">
          <Logo />
        </a>

        <nav className="hidden items-center gap-8 sm:flex">
          {links.map((link) => (
            <a
              key={link.href}
              href={link.href}
              className="text-[15px] text-gray-300 transition-colors hover:text-lavender-50"
            >
              {link.label}
            </a>
          ))}
        </nav>

        <a
          href="#cta"
          className="hidden rounded-full border-[1.5px] border-lime-400 px-5 py-2 text-sm font-semibold text-lime-400 transition-colors hover:bg-lime-400/10 sm:inline-flex"
        >
          Register
        </a>

        <MobileMenu />
      </div>
    </header>
  );
}

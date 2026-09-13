import type { Metadata } from "next";
import { Sora, Inter, Caveat, Chilanka } from "next/font/google";
import "./globals.css";

const sora = Sora({
  variable: "--font-display",
  subsets: ["latin"],
  weight: ["600", "700"],
});

const inter = Inter({
  variable: "--font-sans",
  subsets: ["latin"],
  weight: ["400", "500", "600"],
});

/**
 * The chalk hand for the classroom board.
 *
 * Two faces, because no single family covers both scripts in a handwritten
 * style: Caveat is the Latin hand, Chilanka is the Malayalam one — an actual
 * Malayalam handwriting face, so conjuncts shape correctly (whiteboard-sync.md
 * requires that to be verified after any font change) instead of being faked
 * by slanting a printed face.
 *
 * They are declared here rather than in the board module so Next hosts and
 * preloads them; the renderer only names them in its font stack and waits on
 * `document.fonts.ready` before it measures any text.
 */
const caveat = Caveat({
  variable: "--font-chalk",
  subsets: ["latin"],
  weight: ["400", "600", "700"],
});

const chilanka = Chilanka({
  variable: "--font-chalk-ml",
  subsets: ["malayalam"],
  weight: ["400"],
});

export const metadata: Metadata = {
  title: "Digi Guru — Live AI tutoring, on your syllabus",
  description:
    "Digi Guru is a live, voice-based AI tutor that writes every explanation on the whiteboard before it speaks — strictly from your own textbook, in focused 20-minute sessions.",
};

export default function RootLayout({ children }: LayoutProps<"/">) {
  return (
    <html
      lang="en"
      className={`${sora.variable} ${inter.variable} ${caveat.variable} ${chilanka.variable} h-full antialiased`}
    >
      <body className="min-h-full flex flex-col bg-ink-950 text-lavender-50">{children}</body>
    </html>
  );
}

import type { Metadata } from "next";
import { Sora, Inter, Noto_Sans_Malayalam } from "next/font/google";
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
 * The board face.
 *
 * Noto Sans Malayalam, not a handwriting font. The board has to render Malayalam
 * conjuncts, mathematical symbols, Greek letters and chart labels *correctly* —
 * and a decorative hand that covers one script beautifully and drops everything
 * else is worse than a plain face that draws all of it. Noto is designed for
 * exactly that coverage, and its Malayalam shaping is the reference
 * implementation.
 *
 * It also carries the Latin glyphs, so English inside a Malayalam sentence stays
 * in one typeface instead of switching mid-line — which is how the textbook
 * prints technical terms and how the tutor is told to speak them.
 *
 * Declared here rather than in the board module so Next hosts and preloads it;
 * the renderer only names the family and waits on `document.fonts.ready` before
 * it measures any text.
 */
const notoMalayalam = Noto_Sans_Malayalam({
  variable: "--font-board",
  subsets: ["malayalam", "latin"],
  weight: ["400", "500", "600", "700"],
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
      className={`${sora.variable} ${inter.variable} ${notoMalayalam.variable} h-full antialiased`}
    >
      <body className="min-h-full flex flex-col bg-ink-950 text-lavender-50">{children}</body>
    </html>
  );
}

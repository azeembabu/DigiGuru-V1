import type { Metadata } from "next";
import { Sora, Inter, Noto_Sans_Malayalam } from "next/font/google";
import "./globals.css";

// DESIGN.md §9.3 — Sora (display), Inter (UI/body), Noto Sans Malayalam
// (curriculum content only; not loaded on the marketing routes' critical
// path since it's only needed once a student reaches curriculum content).
const sora = Sora({
  variable: "--font-sora",
  weight: ["600", "700"],
  subsets: ["latin"],
});

const inter = Inter({
  variable: "--font-inter",
  weight: ["400", "500", "600"],
  subsets: ["latin"],
});

const notoMalayalam = Noto_Sans_Malayalam({
  variable: "--font-noto-malayalam",
  weight: ["400", "500", "600", "700"],
  subsets: ["malayalam"],
});

export const metadata: Metadata = {
  title: "Digi Guru — Learn with an AI tutor, live",
  description:
    "Real-time voice lessons from a strict, curriculum-bound AI tutor, synced with a dynamic whiteboard.",
};

export default function RootLayout({ children }: LayoutProps<"/">) {
  return (
    <html
      lang="en"
      className={`${sora.variable} ${inter.variable} ${notoMalayalam.variable} h-full antialiased`}
    >
      <body className="min-h-full flex flex-col font-sans">{children}</body>
    </html>
  );
}

import type { Metadata } from "next";
import { Sora, Inter } from "next/font/google";
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

export const metadata: Metadata = {
  title: "Digi Guru — Live AI tutoring, on your syllabus",
  description:
    "Digi Guru is a live, voice-based AI tutor that writes every explanation on the whiteboard before it speaks — strictly from your own textbook, in focused 20-minute sessions.",
};

export default function RootLayout({ children }: LayoutProps<"/">) {
  return (
    <html lang="en" className={`${sora.variable} ${inter.variable} h-full antialiased`}>
      <body className="min-h-full flex flex-col bg-ink-950 text-lavender-50">{children}</body>
    </html>
  );
}

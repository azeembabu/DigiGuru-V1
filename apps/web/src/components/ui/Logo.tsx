import Image from "next/image";

import emblem from "@/../public/brand/digi-guru-emblem.png";

// The official SGOU "Digi Guru" mark. The source artwork bakes the wordmark
// into the image in navy, which is unreadable on `ink-950`, so only the
// emblem is used as an image and the wordmark is set as live text — white
// "Digi" + brand-gold "Guru", matching the approved dark-background lockup.
// That also keeps the wordmark crisp at any size and searchable/selectable.

export function Logo({
  size = "md",
  stacked = false,
  className = "",
}: {
  size?: "sm" | "md" | "lg";
  /** Emblem above the wordmark, as in the hero lockup. */
  stacked?: boolean;
  className?: string;
}) {
  const emblemPx = size === "lg" ? 72 : size === "md" ? 36 : 28;
  const wordClass =
    size === "lg" ? "text-3xl" : size === "md" ? "text-lg" : "text-base";

  return (
    <span
      className={`inline-flex ${
        stacked ? "flex-col items-start gap-2.5" : "items-center gap-2.5"
      } ${className}`}
    >
      <Image
        src={emblem}
        alt=""
        width={emblemPx}
        height={emblemPx}
        // The emblem is decorative: the wordmark beside it already carries the
        // accessible name, so announcing it twice would be noise.
        aria-hidden="true"
        priority
        className="h-auto w-auto"
        style={{ width: emblemPx, height: "auto" }}
      />
      <span className={`font-display font-bold tracking-tight ${wordClass}`}>
        <span className="text-lavender-50">Digi</span>{" "}
        <span className="text-brand-gold">Guru</span>
      </span>
    </span>
  );
}

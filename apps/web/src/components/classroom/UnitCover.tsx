"use client";

/**
 * The cover image on a unit card: the rasterised first page of the unit's PDF.
 *
 * # Why this is a `fetch` and not an `<img src>`
 *
 * The session is an httpOnly cookie and the gateway is a different origin from
 * the web app, so a browser sends no credentials on an image load — an `<img>`
 * pointed at the cover route would arrive unauthenticated and 401. Fetching
 * the bytes and rendering an object URL is what puts the cookie on the
 * request, and it inherits the client's refresh-and-retry with it.
 *
 * # A missing cover is a normal state
 *
 * Rendering needs a native pdfium library the ingest worker may not have
 * (`crates/rag/src/thumbnail.rs`), and documents uploaded before covers
 * existed have none. So the placeholder is not an error state: it is the
 * second of two ordinary appearances, and it is built to look deliberate —
 * the unit's number, a colour derived from its own id, and the page count.
 * A broken-image icon or an empty grey box would read as a bug.
 */

import { useEffect, useState } from "react";

import { loadUnitCover } from "@/lib/student";

/** Rendered at 2x by the worker; these are the CSS pixels it paints into. */
const COVER_CLASSES =
  "relative h-[128px] w-[96px] shrink-0 overflow-hidden rounded-lg border border-white/10 bg-ink-900";

export function UnitCover({
  documentId,
  title,
  pageCount,
  hasThumbnail,
}: {
  documentId: string;
  title: string;
  pageCount: number;
  /** From the unit list. `false` means do not even ask — there is none. */
  hasThumbnail: boolean;
}) {
  const url = useCoverUrl(documentId, hasThumbnail);

  if (url === null) {
    return <CoverPlaceholder documentId={documentId} title={title} pageCount={pageCount} />;
  }

  return (
    <div className={COVER_CLASSES}>
      {/*
        A plain <img>, not next/image: the source is a blob: URL created in
        this component, so there is nothing for the image optimiser to fetch,
        size, or cache. `alt=""` because the cover is decorative — the unit's
        title is right beside it, and announcing "cover of Unit 3" to a screen
        reader would be the same sentence twice.
      */}
      {/* eslint-disable-next-line @next/next/no-img-element */}
      <img src={url} alt="" className="h-full w-full object-cover object-top" />
    </div>
  );
}

/**
 * Fetch the cover once per document and hand back an object URL.
 *
 * The URL is revoked when the component unmounts or the document changes —
 * an object URL pins its blob in memory until it is, and a student walking
 * through a course would otherwise accumulate every cover they had passed.
 */
function useCoverUrl(documentId: string, hasThumbnail: boolean): string | null {
  const [url, setUrl] = useState<string | null>(null);

  useEffect(() => {
    if (!hasThumbnail) return;

    let active = true;
    let created: string | null = null;

    void loadUnitCover(documentId).then((blob) => {
      if (blob === null) return;
      if (!active) return;
      created = URL.createObjectURL(blob);
      setUrl(created);
    });

    return () => {
      active = false;
      setUrl(null);
      if (created !== null) URL.revokeObjectURL(created);
    };
  }, [documentId, hasThumbnail]);

  return url;
}

/**
 * What a unit looks like with no rendered page: its own number, in a colour
 * that is stable for that unit, over the page count.
 */
function CoverPlaceholder({
  documentId,
  title,
  pageCount,
}: {
  documentId: string;
  title: string;
  pageCount: number;
}) {
  const hue = hueFor(documentId);
  const label = unitLabel(title);

  return (
    <div
      className={`${COVER_CLASSES} flex flex-col items-center justify-center gap-1`}
      style={{
        // Inline rather than Tailwind: the hue is per-unit data, and the
        // alternative is a palette of arbitrary classes kept in sync by hand.
        backgroundImage: `linear-gradient(150deg, hsl(${hue} 46% 26%), hsl(${hue + 24} 40% 16%))`,
      }}
      aria-hidden
    >
      <span className="font-display text-[22px] font-bold leading-none text-white/90">
        {label}
      </span>
      {pageCount > 0 ? (
        <span className="text-[11px] text-white/55">{pageCount} pages</span>
      ) : null}
    </div>
  );
}

/**
 * A stable hue in 0..359 from the document id.
 *
 * Deterministic so the same unit is the same colour on every visit and on
 * every device — that is the whole point of a placeholder a student is meant
 * to recognise. A plain FNV-style walk over the id is enough; this is a
 * colour, not a hash anything depends on.
 */
function hueFor(documentId: string): number {
  let hash = 2166136261;
  for (let i = 0; i < documentId.length; i += 1) {
    hash ^= documentId.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  return Math.abs(hash) % 360;
}

/**
 * The short label for the tile — "3" from "Unit 3 Forest Resources".
 *
 * Falls back to the first character of the title when a unit is not named
 * that way, rather than printing a whole title into a 96px tile.
 */
function unitLabel(title: string): string {
  const match = /\bunit\s*(\d{1,3})\b/i.exec(title);
  if (match) return match[1];
  const first = title.trim().charAt(0).toUpperCase();
  return first === "" ? "?" : first;
}

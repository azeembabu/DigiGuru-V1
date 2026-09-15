"use client";

/**
 * The video step: `/classroom/video?block=…&unit=…&course=…`.
 *
 * A unit with a video saved against it opens here first; a unit without one
 * never reaches this route at all (`SyllabusBrowser` routes straight to the
 * discussion). Everything on this page is ours — the YouTube player underneath
 * is stripped of its own chrome and covered, so the student sees a Digi Guru
 * screen rather than a YouTube one.
 *
 * # This page is a step, never a wall
 *
 * Every failure here — a unit that turns out to have no video, a blocked IFrame
 * API, a video the owner has made un-embeddable, a hand-typed URL — resolves to
 * *going on to the discussion*, because the tutoring session is the lesson and
 * the video is the introduction to it. The one thing this page must never do is
 * strand a student who came to study.
 *
 * `Skip` is deliberately always available for the same reason. A student on a
 * phone on a metered connection, or one who has already watched, is not served
 * by a gate.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";

import { loadSyllabusUnits, type SyllabusUnit } from "@/lib/student";
import { Logo } from "@/components/ui/Logo";
import {
  createBrandFreePlayer,
  formatTime,
  playerErrorMessage,
  PlayerState,
  type PlayerStateValue,
  type YouTubePlayer,
} from "@/classroom/youtube-player";

/** Seconds after the video ends before the student is taken on automatically. */
const AUTO_CONTINUE_SECONDS = 8;

export function UnitVideo() {
  const router = useRouter();
  const params = useSearchParams();
  const blockId = params.get("block");
  const unitId = params.get("unit");
  const courseId = params.get("course");

  const discussionHref = useMemo(() => {
    const search = new URLSearchParams();
    if (blockId) search.set("block", blockId);
    if (unitId) search.set("unit", unitId);
    if (courseId) search.set("course", courseId);
    const qs = search.toString();
    return qs === "" ? "/classroom" : `/classroom/session?${qs}`;
  }, [blockId, unitId, courseId]);

  const backHref = useMemo(() => {
    const search = new URLSearchParams();
    if (courseId) search.set("course", courseId);
    if (blockId) search.set("block", blockId);
    const qs = search.toString();
    return qs === "" ? "/classroom" : `/classroom?${qs}`;
  }, [courseId, blockId]);

  const [unit, setUnit] = useState<SyllabusUnit | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);

  // The unit list is the authorization boundary as well as the data source:
  // `/student/blocks/{id}/units` starts from the caller's own enrolments, so a
  // block that is not theirs is a 404 here and never renders a player.
  useEffect(() => {
    if (!blockId || !unitId) {
      router.replace("/classroom");
      return;
    }
    let active = true;
    loadSyllabusUnits(blockId)
      .then((units) => {
        if (!active) return;
        const found = units.find((u) => u.document_id === unitId) ?? null;
        // A unit with no video has no business on this page — someone typed
        // the URL, or an admin cleared the link between the list and the
        // click. Either way the discussion is where they were going.
        if (found === null || found.video_id === null) {
          router.replace(discussionHref);
          return;
        }
        setUnit(found);
      })
      .catch((error: unknown) => {
        if (!active) return;
        setLoadError(
          error instanceof Error ? error.message : "This unit could not be opened.",
        );
      });
    return () => {
      active = false;
    };
  }, [blockId, unitId, router, discussionHref]);

  if (loadError !== null) {
    return (
      <Shell backHref={backHref}>
        <div className="mx-auto max-w-lg px-4 py-16 text-center">
          <p className="text-[15px] text-rose-300">{loadError}</p>
          <Link
            href={discussionHref}
            className="mt-6 inline-flex rounded-lg bg-indigo-500 px-5 py-2.5 text-[15px] font-semibold text-white hover:bg-indigo-400"
          >
            Go to the discussion
          </Link>
        </div>
      </Shell>
    );
  }

  return (
    <Shell backHref={backHref}>
      {unit === null || unit.video_id === null ? (
        <div className="mx-auto w-full max-w-5xl px-4 py-6 sm:px-6" aria-busy>
          <div className="aspect-video w-full animate-pulse rounded-2xl bg-white/[0.05]" />
        </div>
      ) : (
        <VideoStage
          key={unit.video_id}
          videoId={unit.video_id}
          title={unit.title}
          discussionHref={discussionHref}
        />
      )}
    </Shell>
  );
}

function VideoStage({
  videoId,
  title,
  discussionHref,
}: {
  videoId: string;
  title: string;
  discussionHref: string;
}) {
  const router = useRouter();
  const mountRef = useRef<HTMLDivElement | null>(null);
  const playerRef = useRef<YouTubePlayer | null>(null);

  const [ready, setReady] = useState(false);
  const [state, setState] = useState<PlayerStateValue>(PlayerState.UNSTARTED);
  const [ended, setEnded] = useState(false);
  const [playerError, setPlayerError] = useState<string | null>(null);
  const [position, setPosition] = useState(0);
  const [duration, setDuration] = useState(0);
  const [muted, setMuted] = useState(false);
  // Whether the video has ever played. Distinct from `playing`: the unstarted
  // frame is YouTube's own poster — title, red play button and a "Watch on
  // YouTube" pill — and needs covering outright, while a mid-video pause shows
  // only the title header and can be masked.
  const [hasStarted, setHasStarted] = useState(false);
  const [posterFallback, setPosterFallback] = useState(false);
  const [scrubbing, setScrubbing] = useState(false);
  const [countdown, setCountdown] = useState<number | null>(null);

  // Built once per video. The API attaches an <iframe> by *replacing* the
  // element it is given, so it gets its own node inside a wrapper we keep —
  // React must never own the node the API replaces.
  useEffect(() => {
    const mount = mountRef.current;
    if (mount === null) return;

    let disposed = false;
    const host = document.createElement("div");
    host.className = "absolute inset-0 h-full w-full";
    mount.appendChild(host);

    createBrandFreePlayer(host, videoId, {
      onReady: () => {
        if (!disposed) setReady(true);
      },
      onStateChange: (next) => {
        if (disposed) return;
        setState(next);
        // The countdown is armed here, beside the state change that causes
        // it, rather than in an effect watching `ended`: deriving one piece of
        // state from another in an effect body is a cascading render, and the
        // player's own callback is where this transition actually happens.
        if (next === PlayerState.ENDED) {
          setEnded(true);
          setCountdown(AUTO_CONTINUE_SECONDS);
        }
        // Replaying resets the completion UI, so "watch again" is a real
        // second viewing rather than a video playing under a finished screen.
        if (next === PlayerState.PLAYING) {
          setEnded(false);
          setCountdown(null);
          setHasStarted(true);
        }
      },
      onError: (code) => {
        if (!disposed) setPlayerError(playerErrorMessage(code));
      },
    })
      .then((player) => {
        if (disposed) {
          player.destroy();
          return;
        }
        playerRef.current = player;
      })
      .catch((error: unknown) => {
        if (disposed) return;
        setPlayerError(
          error instanceof Error
            ? error.message
            : "The video player could not be loaded.",
        );
      });

    return () => {
      disposed = true;
      playerRef.current?.destroy();
      playerRef.current = null;
      host.remove();
    };
  }, [videoId]);

  // One timer for the progress bar, running only while the video is.
  useEffect(() => {
    if (!ready || state !== PlayerState.PLAYING) return;
    const id = setInterval(() => {
      const player = playerRef.current;
      if (player === null) return;
      if (!scrubbing) setPosition(player.getCurrentTime());
      setDuration(player.getDuration());
    }, 250);
    return () => clearInterval(id);
  }, [ready, state, scrubbing]);

  // Auto-advance after the video ends, with the count visible and a way out.
  // A silent redirect would leave a student unsure whether they had missed
  // something; a button with no timer leaves the common case as an extra click.
  useEffect(() => {
    if (countdown === null || countdown <= 0) return;
    const id = setInterval(() => {
      setCountdown((n) => (n === null ? null : Math.max(n - 1, 0)));
    }, 1000);
    return () => clearInterval(id);
  }, [countdown]);

  useEffect(() => {
    if (countdown !== null && countdown <= 0) router.push(discussionHref);
  }, [countdown, router, discussionHref]);

  const playing = state === PlayerState.PLAYING;

  const togglePlay = useCallback(() => {
    const player = playerRef.current;
    if (player === null) return;
    if (player.getPlayerState() === PlayerState.PLAYING) player.pauseVideo();
    else player.playVideo();
  }, []);

  const toggleMute = useCallback(() => {
    const player = playerRef.current;
    if (player === null) return;
    if (player.isMuted()) {
      player.unMute();
      setMuted(false);
    } else {
      player.mute();
      setMuted(true);
    }
  }, []);

  const seek = useCallback((seconds: number) => {
    playerRef.current?.seekTo(seconds, true);
    setPosition(seconds);
  }, []);

  const nudge = useCallback(
    (delta: number) => {
      const player = playerRef.current;
      if (player === null) return;
      const next = Math.min(
        Math.max(player.getCurrentTime() + delta, 0),
        player.getDuration() || Number.MAX_SAFE_INTEGER,
      );
      seek(next);
    },
    [seek],
  );

  // Keyboard control, since YouTube's own is disabled (`disablekb`). Ignored
  // while the student is typing somewhere, so it never eats a keystroke.
  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      const target = event.target as HTMLElement | null;
      if (
        target &&
        (target.isContentEditable ||
          ["INPUT", "TEXTAREA", "SELECT", "BUTTON"].includes(target.tagName))
      ) {
        return;
      }
      if (event.key === " " || event.key === "k") {
        event.preventDefault();
        togglePlay();
      } else if (event.key === "ArrowRight") {
        event.preventDefault();
        nudge(5);
      } else if (event.key === "ArrowLeft") {
        event.preventDefault();
        nudge(-5);
      } else if (event.key === "m") {
        toggleMute();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [togglePlay, nudge, toggleMute]);

  const progress = duration > 0 ? Math.min(position / duration, 1) : 0;

  return (
    <div className="mx-auto w-full max-w-5xl px-4 py-5 sm:px-6 sm:py-8">
      <p className="text-[12px] font-semibold uppercase tracking-[0.14em] text-gray-500">
        Before the discussion
      </p>
      <h1 className="mt-1 text-balance font-display text-[22px] font-bold leading-tight text-white sm:text-[26px]">
        {title}
      </h1>

      {/*
        `aspect-video` on the frame plus an absolutely positioned player is what
        keeps this correct at every width without a resize listener: the frame
        is as wide as the column and as tall as 16:9 makes it, so a phone in
        portrait gets a full-width video with no letterboxing and no overflow.
      */}
      <div className="mt-4 overflow-hidden rounded-2xl border border-white/10 bg-black shadow-2xl">
        <div className="relative aspect-video w-full">
          <div ref={mountRef} className="absolute inset-0" />

          {/*
            The click-catcher. Two jobs: it makes the whole frame a play/pause
            target the way a real player is, and — the reason it is not
            optional — it sits above the iframe so YouTube's own remaining hit
            targets (the watermark link, the title that appears on pause, the
            share overlay) cannot be clicked out of our page. Marked
            presentational because the real control is the labelled button in
            the bar below; this is an affordance, not the only way through.
          */}
          <button
            type="button"
            aria-hidden
            tabIndex={-1}
            onClick={togglePlay}
            className="absolute inset-0 h-full w-full cursor-pointer bg-transparent"
          />

          {!ready && playerError === null ? (
            <div className="pointer-events-none absolute inset-0 grid place-items-center bg-ink-950/80">
              <div className="h-10 w-10 animate-spin rounded-full border-2 border-white/20 border-t-white/80" />
            </div>
          ) : null}

          {/*
            Our own poster, over YouTube's.

            An unstarted embed is not a still frame: YouTube paints the video
            title, the channel avatar, a large red play button and a "Watch on
            YouTube" pill over it, and no player parameter suppresses any of
            them. So the whole frame is covered with the same thumbnail and our
            own play control — the student sees the video, not the product it
            happens to be hosted on. Once it has started, this is gone and only
            the corner watermark remains, which YouTube's terms do not allow
            removing anyway.
          */}
          {!hasStarted && !ended && playerError === null ? (
            /* eslint-disable-next-line @next/next/no-img-element -- a YouTube
               thumbnail is a remote URL on a host the app does not configure for
               the image optimiser; proxying every poster through the app would
               buy nothing. */
            <img
              src={`https://i.ytimg.com/vi/${videoId}/${posterFallback ? "hqdefault" : "maxresdefault"}.jpg`}
              alt=""
              aria-hidden
              // `maxresdefault` does not exist for every video; `hqdefault`
              // always does.
              onError={() => setPosterFallback(true)}
              className="pointer-events-none absolute inset-0 h-full w-full object-cover"
            />
          ) : null}

          {/*
            A paused video shows the title header and share button across the
            top instead. `showinfo` was retired, so it is masked with a band in
            our own colour — deep enough to cover the header, shallow enough to
            read as a vignette rather than a crop.
          */}
          {ready && hasStarted && !playing && playerError === null ? (
            <div className="pointer-events-none absolute inset-x-0 top-0 h-20 bg-gradient-to-b from-ink-950 via-ink-950/90 to-transparent" />
          ) : null}

          {/* Big centre play button, hidden once the video is running. */}
          {ready && !playing && !ended && playerError === null ? (
            <button
              type="button"
              onClick={togglePlay}
              className={`absolute inset-0 grid place-items-center transition focus-visible:outline-none ${
                hasStarted ? "bg-ink-950/40 hover:bg-ink-950/30" : "bg-ink-950/45 hover:bg-ink-950/35"
              }`}
              aria-label="Play the video"
            >
              <span className="grid h-16 w-16 place-items-center rounded-full bg-white/95 shadow-xl sm:h-20 sm:w-20">
                <PlayIcon className="ml-1 h-7 w-7 text-ink-950 sm:h-9 sm:w-9" />
              </span>
            </button>
          ) : null}

          {/*
            The completion screen. Fully opaque, not a tint: when a video ends
            YouTube paints its own end card underneath — the title, the channel
            avatar, a "More videos" grid and the logo — and at 90% every one of
            them was legible through this. Opacity here is branding leaking
            back in at the exact moment the student is being handed on.
          */}
          {ended ? (
            <div className="absolute inset-0 grid place-items-center bg-ink-950 px-4 text-center">
              <div>
                <p className="font-display text-[20px] font-bold text-white sm:text-[24px]">
                  Video finished
                </p>
                <p className="mx-auto mt-2 max-w-sm text-[14px] leading-relaxed text-gray-400">
                  Now talk it through with your tutor — it teaches from this
                  unit and writes on the board as it goes.
                </p>
                <div className="mt-6 flex flex-wrap items-center justify-center gap-3">
                  <Link
                    href={discussionHref}
                    className="rounded-lg bg-indigo-500 px-6 py-3 text-[15px] font-semibold text-white shadow-lg hover:bg-indigo-400 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-300"
                  >
                    Continue to Interactive Discussion
                    {countdown !== null && countdown > 0 ? ` (${countdown})` : ""}
                  </Link>
                  <button
                    type="button"
                    onClick={() => {
                      setCountdown(null);
                      setEnded(false);
                      setHasStarted(true);
                      seek(0);
                      playerRef.current?.playVideo();
                    }}
                    className="rounded-lg border border-white/15 px-5 py-3 text-[14px] font-medium text-gray-300 hover:border-white/30 hover:text-white"
                  >
                    Watch again
                  </button>
                </div>
              </div>
            </div>
          ) : null}

          {playerError !== null ? (
            <div className="absolute inset-0 grid place-items-center bg-ink-950 px-4 text-center">
              <div>
                <p className="mx-auto max-w-sm text-[14px] leading-relaxed text-rose-300">
                  {playerError}
                </p>
                <Link
                  href={discussionHref}
                  className="mt-5 inline-flex rounded-lg bg-indigo-500 px-5 py-2.5 text-[15px] font-semibold text-white hover:bg-indigo-400"
                >
                  Continue to Interactive Discussion
                </Link>
              </div>
            </div>
          ) : null}
        </div>

        {/* Our control bar. YouTube's is off (`controls: 0`). */}
        <div className="flex items-center gap-3 border-t border-white/10 bg-ink-950 px-3 py-2.5 sm:px-4">
          <button
            type="button"
            onClick={togglePlay}
            disabled={!ready}
            aria-label={playing ? "Pause" : "Play"}
            className="grid h-9 w-9 shrink-0 place-items-center rounded-full bg-white/10 text-white transition hover:bg-white/20 disabled:opacity-40"
          >
            {playing ? <PauseIcon className="h-4 w-4" /> : <PlayIcon className="ml-0.5 h-4 w-4" />}
          </button>

          <span className="hidden shrink-0 text-[12px] tabular-nums text-gray-400 sm:inline">
            {formatTime(position)}
          </span>

          {/*
            A range input rather than a div with a drag handler: it is keyboard
            operable and screen-reader labelled for free, which a bespoke
            scrubber would have to re-earn.
          */}
          <input
            type="range"
            min={0}
            max={duration > 0 ? duration : 100}
            step={0.5}
            value={position}
            disabled={!ready || duration === 0}
            aria-label="Seek"
            onMouseDown={() => setScrubbing(true)}
            onTouchStart={() => setScrubbing(true)}
            onChange={(event) => setPosition(Number(event.target.value))}
            onMouseUp={(event) => {
              setScrubbing(false);
              seek(Number(event.currentTarget.value));
            }}
            onTouchEnd={(event) => {
              setScrubbing(false);
              seek(Number(event.currentTarget.value));
            }}
            onKeyUp={(event) => seek(Number(event.currentTarget.value))}
            className="h-1.5 w-full cursor-pointer appearance-none rounded-full bg-white/15 accent-indigo-400 disabled:cursor-default [&::-webkit-slider-thumb]:h-3.5 [&::-webkit-slider-thumb]:w-3.5 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-indigo-400"
            style={{
              backgroundImage: `linear-gradient(to right, rgb(129 140 248) ${progress * 100}%, rgba(255,255,255,0.15) ${progress * 100}%)`,
            }}
          />

          <span className="shrink-0 text-[12px] tabular-nums text-gray-400">
            {formatTime(duration)}
          </span>

          <button
            type="button"
            onClick={toggleMute}
            disabled={!ready}
            aria-label={muted ? "Unmute" : "Mute"}
            className="grid h-9 w-9 shrink-0 place-items-center rounded-full text-gray-300 transition hover:bg-white/10 hover:text-white disabled:opacity-40"
          >
            {muted ? <MutedIcon className="h-4 w-4" /> : <SoundIcon className="h-4 w-4" />}
          </button>
        </div>
      </div>

      <div className="mt-5 flex flex-wrap items-center justify-between gap-3">
        <p className="text-[13px] text-gray-500">
          {ended
            ? "You can rewatch this any time from your syllabus."
            : "Watch this first — the tutor picks up from here."}
        </p>
        {/*
          Always available, and always secondary to the completion button. A
          student on a metered phone connection, or one who has already watched
          this unit, must not have to sit through it again to reach the tutor.
        */}
        <Link
          href={discussionHref}
          className="rounded-lg border border-white/15 px-4 py-2 text-[14px] font-medium text-gray-300 transition hover:border-white/30 hover:text-white"
        >
          Skip to discussion →
        </Link>
      </div>
    </div>
  );
}

function Shell({ backHref, children }: { backHref: string; children: React.ReactNode }) {
  return (
    <main className="min-h-dvh bg-ink-950 text-lavender-50">
      <header className="flex flex-wrap items-center gap-x-4 gap-y-2 border-b border-white/10 px-4 py-3 sm:px-6">
        <Logo size="sm" />
        <Link href={backHref} className="text-[13px] text-gray-400 hover:text-gray-200">
          ← Syllabus
        </Link>
        <Link
          href="/dashboard"
          className="ml-auto text-[13px] text-gray-400 hover:text-gray-200"
        >
          Dashboard
        </Link>
      </header>
      {children}
    </main>
  );
}

function PlayIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden className={className}>
      <path d="M8 5.14v13.72a1 1 0 0 0 1.54.84l10.3-6.86a1 1 0 0 0 0-1.68L9.54 4.3A1 1 0 0 0 8 5.14Z" />
    </svg>
  );
}

function PauseIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden className={className}>
      <path d="M7 4h3.5v16H7zM13.5 4H17v16h-3.5z" />
    </svg>
  );
}

function SoundIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden className={className}>
      <path d="M4 9v6h3.5L12 19.5v-15L7.5 9H4Zm12.5 3a4 4 0 0 0-2-3.46v6.92A4 4 0 0 0 16.5 12Zm-2 7.9a6.5 6.5 0 0 0 0-15.8v1.6a5 5 0 0 1 0 12.6v1.6Z" />
    </svg>
  );
}

function MutedIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden className={className}>
      <path d="M4 9v6h3.5L12 19.5v-15L7.5 9H4Zm11.3 3 2.9-2.9-1.4-1.4-2.9 2.9-2.9-2.9v2.8l1.5 1.5-1.5 1.5v2.8l2.9-2.9 2.9 2.9 1.4-1.4-2.9-2.9Z" />
    </svg>
  );
}

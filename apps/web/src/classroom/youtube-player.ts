/**
 * A thin, framework-agnostic wrapper over the YouTube IFrame API.
 *
 * # Why the IFrame API and not a plain `<iframe src=…>`
 *
 * Two things a static embed cannot do, both of them requirements here: tell us
 * the video has **finished** (there is no such event without the API), and let
 * us drive playback from our own controls. Native YouTube controls are turned
 * off entirely — the player is chrome-free and everything the student sees is
 * ours — which is only workable if we can start, pause and seek it ourselves.
 *
 * # Why no player library
 *
 * Plyr and its peers wrap exactly this API to do exactly this. The parts we
 * need are `onStateChange`, `onReady` and half a dozen methods, so a dependency
 * would buy skinning we are doing ourselves anyway, against a design system it
 * does not know about. Kept dependency-free deliberately.
 *
 * # What "brand-free" actually means
 *
 * YouTube's terms do not allow removing the watermark that appears over a
 * *playing* video, and no embed parameter removes it — `modestbranding` was
 * retired. What is removable, and is removed here, is everything around it:
 * the control bar, the title header, the channel avatar, the share and
 * watch-later buttons, the annotations, the keyboard shortcuts and — the one
 * that actually matters pedagogically — the grid of unrelated suggested videos
 * at the end, which `rel=0` plus stopping on `ENDED` prevents. The player is
 * then covered by our own click-catching surface, so none of YouTube's
 * remaining hit targets can be reached even where they are still painted.
 *
 * No React in this file, per `.claude/rules/code-style.md`: the classroom's
 * media code stays testable without a renderer.
 */

/** The subset of `YT.Player` this app uses. */
export interface YouTubePlayer {
  playVideo(): void;
  pauseVideo(): void;
  seekTo(seconds: number, allowSeekAhead: boolean): void;
  mute(): void;
  unMute(): void;
  isMuted(): boolean;
  setVolume(volume: number): void;
  getVolume(): number;
  getCurrentTime(): number;
  getDuration(): number;
  getPlayerState(): number;
  destroy(): void;
}

/** `YT.PlayerState`, inlined so nothing depends on the global being loaded. */
export const PlayerState = {
  UNSTARTED: -1,
  ENDED: 0,
  PLAYING: 1,
  PAUSED: 2,
  BUFFERING: 3,
  CUED: 5,
} as const;

export type PlayerStateValue = (typeof PlayerState)[keyof typeof PlayerState];

interface YouTubeApi {
  Player: new (
    element: HTMLElement | string,
    options: {
      videoId: string;
      playerVars: Record<string, string | number>;
      events: {
        onReady?: () => void;
        onStateChange?: (event: { data: number }) => void;
        onError?: (event: { data: number }) => void;
      };
    },
  ) => YouTubePlayer;
}

declare global {
  interface Window {
    YT?: YouTubeApi;
    onYouTubeIframeAPIReady?: () => void;
  }
}

const API_SRC = "https://www.youtube.com/iframe_api";

let apiPromise: Promise<YouTubeApi> | null = null;

/**
 * Load the IFrame API once per document.
 *
 * The API calls a **single global** callback when it is ready, so a second
 * loader that overwrote `onYouTubeIframeAPIReady` would silently strand the
 * first. Hence the module-level promise: every caller awaits the same load.
 */
export function loadYouTubeApi(): Promise<YouTubeApi> {
  if (typeof window === "undefined") {
    return Promise.reject(new Error("The video player needs a browser."));
  }
  if (window.YT?.Player) return Promise.resolve(window.YT);
  if (apiPromise) return apiPromise;

  const pending = new Promise<YouTubeApi>((resolve, reject) => {
    const previous = window.onYouTubeIframeAPIReady;
    window.onYouTubeIframeAPIReady = () => {
      previous?.();
      if (window.YT?.Player) resolve(window.YT);
      else reject(new Error("The video player failed to load."));
    };

    if (!document.querySelector(`script[src="${API_SRC}"]`)) {
      const script = document.createElement("script");
      script.src = API_SRC;
      script.async = true;
      // A blocked script (an extension, a filtered network, an offline device)
      // must surface as a rejection rather than as a player that never appears:
      // the page turns that into "skip to the discussion", which keeps the
      // lesson reachable.
      script.onerror = () => reject(new Error("The video player could not be loaded."));
      document.head.appendChild(script);
    }
  });

  // A failed load must not be cached as the answer for the rest of the session
  // — reloading the page should be able to try again.
  apiPromise = pending;
  pending.catch(() => {
    apiPromise = null;
  });
  return pending;
}

/**
 * Player parameters that strip every piece of YouTube UI that can be stripped.
 *
 * `controls: 0` is what makes the rest of this file necessary — with YouTube's
 * own bar gone, the app owns play, seek, volume and fullscreen.
 */
export function brandFreePlayerVars(origin: string | null): Record<string, string | number> {
  return {
    controls: 0, // no YouTube control bar — ours replaces it
    disablekb: 1, // our own key handling, so space never double-fires
    fs: 0, // fullscreen is the wrapper's, not the iframe's
    iv_load_policy: 3, // no annotations
    modestbranding: 1, // retired by YouTube, harmless, and honest about intent
    rel: 0, // related videos, where still shown, stay within this channel
    playsinline: 1, // iOS: play in the page rather than taking over the screen
    showinfo: 0, // legacy title/uploader header
    cc_load_policy: 0,
    // Same-origin check on the API's postMessage channel; omitted where there
    // is no location to declare.
    ...(origin ? { origin } : {}),
  };
}

/** Create a chrome-free player bound to `container`. */
export async function createBrandFreePlayer(
  container: HTMLElement,
  videoId: string,
  events: {
    onReady?: () => void;
    onStateChange?: (state: PlayerStateValue) => void;
    onError?: (code: number) => void;
  },
): Promise<YouTubePlayer> {
  const api = await loadYouTubeApi();
  const origin = typeof window === "undefined" ? null : window.location.origin;
  return new api.Player(container, {
    videoId,
    playerVars: brandFreePlayerVars(origin),
    events: {
      onReady: events.onReady,
      onStateChange: (event) => events.onStateChange?.(event.data as PlayerStateValue),
      onError: (event) => events.onError?.(event.data),
    },
  });
}

/**
 * What an `onError` code means, in words a student can act on.
 *
 * The codes are YouTube's; the distinction that matters to a student is "this
 * video cannot be played here, and that is not your fault" — every message ends
 * by pointing at the discussion, because the lesson is still reachable.
 */
export function playerErrorMessage(code: number): string {
  switch (code) {
    case 2:
      return "That video link is not valid. Tell your centre, and carry on to the discussion.";
    case 5:
      return "This video cannot play in this browser. Carry on to the discussion.";
    case 100:
      return "That video is no longer available. Tell your centre, and carry on to the discussion.";
    case 101:
    case 150:
      return "The video's owner does not allow it to play outside YouTube. Carry on to the discussion.";
    default:
      return "The video could not be played. Carry on to the discussion.";
  }
}

/** `m:ss`, or `h:mm:ss` past an hour. `0:00` for an unknown duration. */
export function formatTime(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "0:00";
  const total = Math.floor(seconds);
  const s = total % 60;
  const m = Math.floor(total / 60) % 60;
  const h = Math.floor(total / 3600);
  const mm = h > 0 ? String(m).padStart(2, "0") : String(m);
  return `${h > 0 ? `${h}:` : ""}${mm}:${String(s).padStart(2, "0")}`;
}

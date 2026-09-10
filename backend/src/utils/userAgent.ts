/**
 * Minimal user-agent summariser for the Active Sessions list.
 *
 * We return a short human label ("Chrome on Windows") and deliberately never
 * send the raw user-agent string to the client: it is noisy, and the client has
 * no use for it beyond recognising its own sessions.
 *
 * Order matters — Edge and Opera both include "Chrome" in their UA, and Android
 * includes "Linux", so the more specific patterns are tested first.
 */

const BROWSERS: ReadonlyArray<{ test: RegExp; label: string }> = [
  { test: /Edg\//, label: 'Edge' },
  { test: /OPR\/|Opera/, label: 'Opera' },
  { test: /Chrome\//, label: 'Chrome' },
  { test: /Firefox\//, label: 'Firefox' },
  { test: /Safari\//, label: 'Safari' },
  { test: /curl|wget|PostmanRuntime|node-fetch|axios/i, label: 'API client' },
];

const PLATFORMS: ReadonlyArray<{ test: RegExp; label: string }> = [
  { test: /Windows/, label: 'Windows' },
  { test: /Android/, label: 'Android' },
  { test: /iPhone|iPad|iPod/, label: 'iOS' },
  { test: /Mac OS X|Macintosh/, label: 'macOS' },
  { test: /Linux/, label: 'Linux' },
];

export function describeDevice(userAgent?: string | null): string {
  if (!userAgent) return 'Unknown device';

  const browser = BROWSERS.find((b) => b.test.test(userAgent))?.label;
  const platform = PLATFORMS.find((p) => p.test.test(userAgent))?.label;

  if (browser && platform) return `${browser} on ${platform}`;
  return browser ?? platform ?? 'Unknown device';
}

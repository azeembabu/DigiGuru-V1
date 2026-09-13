import { TONES, clamp01, formatPercent, type Tone } from "./palette";

type ProgressBarProps = {
  value: number;
  label?: string;
  tone?: Tone;
};

/**
 * A 0..1 ratio. `clamp01` also absorbs `NaN` (the shape of `0 / 0`, which is
 * what an empty database hands a caller computing a completion rate) so the
 * bar renders at zero rather than vanishing.
 */
export function ProgressBar({ value, label, tone = "default" }: ProgressBarProps) {
  const ratio = clamp01(value);
  const { fill, text, track, word } = TONES[tone];
  const pct = formatPercent(ratio);

  return (
    <div>
      {label ? (
        <div className="mb-1.5 flex items-baseline justify-between gap-3">
          <span className="text-sm text-gray-900">{label}</span>
          {/* The percentage is always written out — the fill colour alone is
              neither readable at a glance nor available to a screen reader. */}
          <span className="text-sm font-medium tabular-nums" style={{ color: text }}>
            {pct}
            {word ? <span className="sr-only"> — {word}</span> : null}
          </span>
        </div>
      ) : null}
      <div
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(ratio * 100)}
        aria-valuetext={pct}
        aria-label={label ?? "Progress"}
        className="h-2 w-full overflow-hidden rounded-full"
        style={{ background: track }}
      >
        <div className="h-full rounded-full transition-[width]" style={{ width: `${ratio * 100}%`, background: fill }} />
      </div>
    </div>
  );
}

import { format_gb } from '$lib/instances';

const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? '' : 's'}`;

/** Header text like `3 games running across 2 profiles.` */
export const running_summary = (games: number, profiles: number) =>
  `${plural(games, 'game')} running across ${plural(profiles, 'profile')}.`;

/** Memory label and bar percent, undefined when the usage is unknown */
export const memory_usage = (used_bytes?: number, total_mb?: number) => {
  if (used_bytes === undefined) {
    return undefined;
  }
  const total_bytes = (total_mb ?? 0) * 1024 * 1024;
  return {
    label: total_bytes
      ? `${format_gb(used_bytes)} of ${format_gb(total_bytes)} GB`
      : `${format_gb(used_bytes)} GB`,
    percent: total_bytes
      ? Math.min(100, Math.round((used_bytes / total_bytes) * 100))
      : 0
  };
};

/** `Playing as X · running for 5m`, parts omitted when unknown */
export const instance_subtitle = (
  uptime: string,
  account?: string,
  playing_as = false
) =>
  [
    account && (playing_as ? `Playing as ${account}` : account),
    `running for ${uptime}`
  ]
    .filter(Boolean)
    .join(' · ');

import { LoaderType, type Profile } from './tauri/profile.svelte';
import type { QuickPlayInfo } from './tauri/quick-play.svelte';

export const plural = (n: number, word: string) =>
  `${n} ${word}${n === 1 ? '' : 's'}`;

/** Muted meta line of a profile card, e.g. `24 mods · played 2 hours ago` */
export const card_meta = (opts: {
  loader: LoaderType;
  mods?: number;
  /** Relative last played time, e.g. `2 hours ago` */
  played?: string;
  /** Set for quick play entries, replaces the mod count */
  profile_name?: string;
}) => {
  const parts: string[] = [];
  if (opts.profile_name !== undefined) {
    parts.push(opts.profile_name);
  } else if (opts.loader !== LoaderType.Vanilla && opts.mods !== undefined) {
    parts.push(plural(opts.mods, 'mod'));
  }
  parts.push(opts.played ? `played ${opts.played}` : 'never played');
  return parts.join(' · ');
};

/** Most recently played profile, undefined if none was ever played */
export const latest_played = (profiles: Profile[]) =>
  profiles
    .filter((p) => p.last_played)
    .toSorted(
      (a, b) =>
        Date.parse(b.last_played ?? '') - Date.parse(a.last_played ?? '')
    )[0];

/**
 * World or server to resume when it was played after the last plain launch.
 * Entries only exist for versions that support quick play.
 */
export const continue_quick_play = (
  profile: Profile
): QuickPlayInfo | undefined => {
  const [last] = (profile.quick_play ?? []).toSorted(
    (a, b) => Date.parse(b.lastPlayedTime) - Date.parse(a.lastPlayedTime)
  );
  if (!last) {
    return undefined;
  }
  const plain = profile.last_played_non_quick_play;
  return !plain || Date.parse(last.lastPlayedTime) > Date.parse(plain)
    ? last
    : undefined;
};

export const profiles_summary = (profiles: Profile[]) =>
  `${plural(profiles.length, 'profile')} · ${plural(
    profiles.filter((p) => p.favorite).length,
    'favorite'
  )}`;

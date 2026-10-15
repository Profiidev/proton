import { describe, expect, it } from 'vitest';
import {
  card_meta,
  continue_quick_play,
  latest_played,
  profiles_summary
} from '$lib/profiles';
import {
  type QuickPlayInfo,
  QuickPlayType
} from '$lib/tauri/quick-play.svelte';
import { LoaderType, type Profile } from '$lib/tauri/profile.svelte';

const profile = (id: string, last_played?: string, favorite = false) =>
  ({
    created_at: '2024-01-01T00:00:00Z',
    favorite,
    id,
    last_played,
    loader: LoaderType.Fabric,
    name: id,
    use_local_game: false,
    use_local_jvm: false,
    version: '1.21'
  }) satisfies Profile;

describe('card_meta', () => {
  it('shows mods and played time', () => {
    expect(
      card_meta({ loader: LoaderType.Fabric, mods: 24, played: '2 hours ago' })
    ).toBe('24 mods · played 2 hours ago');
    expect(card_meta({ loader: LoaderType.Forge, mods: 1 })).toBe(
      '1 mod · never played'
    );
  });

  it('omits mods for vanilla and unknown counts', () => {
    expect(
      card_meta({ loader: LoaderType.Vanilla, mods: 3, played: 'yesterday' })
    ).toBe('played yesterday');
    expect(card_meta({ loader: LoaderType.Quilt })).toBe('never played');
  });

  it('shows the profile name for quick play entries', () => {
    expect(
      card_meta({
        loader: LoaderType.Fabric,
        mods: 5,
        played: 'yesterday',
        profile_name: 'SMP'
      })
    ).toBe('SMP · played yesterday');
  });
});

describe('latest_played', () => {
  it('picks the most recently played profile', () => {
    const profiles = [
      profile('a', '2024-05-01T10:00:00Z'),
      profile('b'),
      profile('c', '2024-05-02T09:00:00+02:00'),
      profile('d', '2024-04-30T00:00:00Z')
    ];
    expect(latest_played(profiles)?.id).toBe('c');
  });

  it('returns undefined when nothing was played', () => {
    expect(latest_played([profile('a')])).toBeUndefined();
    expect(latest_played([])).toBeUndefined();
  });
});

describe('profiles_summary', () => {
  it('counts profiles and favorites', () => {
    expect(
      profiles_summary([profile('a', undefined, true), profile('b')])
    ).toBe('2 profiles · 1 favorite');
    expect(profiles_summary([])).toBe('0 profiles · 0 favorites');
  });
});

const entry = (id: string, lastPlayedTime: string) =>
  ({
    favorite: false,
    history: true,
    id,
    lastPlayedTime,
    name: id,
    type: QuickPlayType.Multiplayer
  }) satisfies QuickPlayInfo;

describe('continue_quick_play', () => {
  it('picks the latest world or server played after the last plain launch', () => {
    const p = {
      ...profile('a', '2024-01-03T00:00:00Z'),
      last_played_non_quick_play: '2024-01-01T00:00:00Z',
      quick_play: [
        entry('old', '2024-01-02T00:00:00Z'),
        entry('new', '2024-01-03T00:00:00Z')
      ]
    };
    expect(continue_quick_play(p)?.id).toBe('new');
  });

  it('keeps the plain profile when it was launched last', () => {
    const p = {
      ...profile('a', '2024-01-04T00:00:00Z'),
      last_played_non_quick_play: '2024-01-04T00:00:00Z',
      quick_play: [entry('old', '2024-01-02T00:00:00Z')]
    };
    expect(continue_quick_play(p)).toBeUndefined();
  });

  it('returns undefined without quick play entries', () => {
    expect(continue_quick_play(profile('a'))).toBeUndefined();
  });
});

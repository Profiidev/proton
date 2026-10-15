import { describe, expect, it } from 'vitest';
import {
  players_label,
  show_note
} from '../../../../../../routes/profiles/info/quick_play/quick-play';
import type { QuickPlayInfo } from '$lib/tauri/quick-play.svelte';

describe('players_label', () => {
  it('hides while unknown', () => {
    expect(players_label(undefined)).toBeUndefined();
  });
  it('shows offline', () => {
    expect(
      players_label({ online: false, players_max: 0, players_online: 0 })
    ).toBe('Offline');
  });
  it('shows player counts', () => {
    expect(
      players_label({ online: true, players_max: 20, players_online: 3 })
    ).toBe('3 / 20 online');
  });
});

describe('show_note', () => {
  // oxlint-disable-next-line no-unsafe-type-assertion
  const entry = {} as QuickPlayInfo;
  it('shows when no worlds and no servers', () => {
    expect(show_note(undefined, [])).toBe(true);
  });
  it('hides when any world or server exists', () => {
    expect(show_note([entry], [])).toBe(false);
    expect(show_note(undefined, [entry])).toBe(false);
  });
});

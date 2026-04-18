import { UpdateType, create_data_state } from '$lib/data_state.svelte';
import type { Profile } from '$lib/tauri/profile.svelte';
import {
  type QuickPlayInfo,
  QuickPlayType,
  profile_quick_play_list
} from '$lib/tauri/quick_play.svelte';
import { compareDateTimes } from '$lib/util.svelte';

let singleplayer = $state<QuickPlayInfo[] | undefined>();
let multiplayer = $state<QuickPlayInfo[] | undefined>();
let realms = $state<QuickPlayInfo[] | undefined>();

export const quick_play_updater = (profile?: Profile) => (
    profile &&
    create_data_state(async () => {
      const quick_play_list = await profile_quick_play_list(profile.id);

      singleplayer = quick_play_list
        ?.filter((q) => q.type === QuickPlayType.Singleplayer)
        .toSorted((a, b) => compareDateTimes(a.lastPlayedTime, b.lastPlayedTime));

      multiplayer = quick_play_list
        ?.filter((q) => q.type === QuickPlayType.Multiplayer)
        .toSorted((a, b) => compareDateTimes(a.lastPlayedTime, b.lastPlayedTime));

      realms = quick_play_list
        ?.filter((q) => q.type === QuickPlayType.Realms)
        .toSorted((a, b) => compareDateTimes(a.lastPlayedTime, b.lastPlayedTime));

      return quick_play_list;
    }, UpdateType.ProfileQuickPlay)
  );

export const singleplayer_list = () => singleplayer;

export const multiplayer_list = () => multiplayer;

export const realms_list = () => realms;

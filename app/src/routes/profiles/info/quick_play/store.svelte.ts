import { create_data_state, UpdateType } from '$lib/data_state.svelte';
import type { Profile } from '$lib/tauri/profile.svelte';
import {
  profile_quick_play_list,
  QuickPlayType,
  type QuickPlayInfo
} from '$lib/tauri/quick_play.svelte';
import { compareDateTimes } from '$lib/util.svelte';

let singleplayer = $state<QuickPlayInfo[] | undefined>();
let multiplayer = $state<QuickPlayInfo[] | undefined>();
let realms = $state<QuickPlayInfo[] | undefined>();

export let quick_play_updater = (profile?: Profile) => {
  return (
    profile &&
    create_data_state(async () => {
      let quick_play_list = await profile_quick_play_list(profile.id);

      singleplayer = quick_play_list
        ?.filter((q) => q.type === QuickPlayType.Singleplayer)
        .sort((a, b) => compareDateTimes(a.lastPlayedTime, b.lastPlayedTime));

      multiplayer = quick_play_list
        ?.filter((q) => q.type === QuickPlayType.Multiplayer)
        .sort((a, b) => compareDateTimes(a.lastPlayedTime, b.lastPlayedTime));

      realms = quick_play_list
        ?.filter((q) => q.type === QuickPlayType.Realms)
        .sort((a, b) => compareDateTimes(a.lastPlayedTime, b.lastPlayedTime));

      return quick_play_list;
    }, UpdateType.ProfileQuickPlay)
  );
};

export const singleplayer_list = () => {
  return singleplayer;
};

export const multiplayer_list = () => {
  return multiplayer;
};

export const realms_list = () => {
  return realms;
};

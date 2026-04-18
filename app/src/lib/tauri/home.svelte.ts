import { UpdateType, create_data_state } from '$lib/data_state.svelte';
import { invoke } from '@tauri-apps/api/core';
import { type Profile, type ProfileError, parseError } from './profile.svelte';
import type { QuickPlayInfo } from './quick_play.svelte';

export interface PlayHistoryFavorite {
  profile: Profile;
  quick_play?: QuickPlayInfo;
}

const profile_history_list_ = async (): Promise<
  PlayHistoryFavorite[] | undefined
> => {
  try {
    return await invoke('profile_history_list');
  } catch {
    return undefined;
  }
};
export const profile_history_list = create_data_state(
  profile_history_list_,
  UpdateType.Profiles
);

const profile_favorites_list_ = async (): Promise<
  PlayHistoryFavorite[] | undefined
> => {
  try {
    return await invoke('profile_favorites_list');
  } catch {
    return undefined;
  }
};
export const profile_favorites_list = create_data_state(
  profile_favorites_list_,
  UpdateType.Profiles
);

export const profile_favorites_set = async (
  profile: string,
  favorite: boolean,
  quickPlay?: QuickPlayInfo
): Promise<undefined | ProfileError> => {
  try {
    await invoke('profile_favorites_set', {
      favorite,
      profile,
      quickPlay
    });
  } catch (error: any) {
    return parseError(error);
  }
  return undefined;
};

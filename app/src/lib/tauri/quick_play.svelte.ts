import { invoke } from '@tauri-apps/api/core';
import { parseError, type ProfileError } from './profile.svelte';

export enum QuickPlayType {
  Singleplayer = 'singleplayer',
  Multiplayer = 'multiplayer',
  Realms = 'realms'
}

export interface QuickPlayInfo {
  type: QuickPlayType;
  name: string;
  id: string;
  lastPlayedTime: string;
  favorite: boolean;
  history: boolean;
}

export const profile_quick_play_list = async (
  profile: string
): Promise<QuickPlayInfo[] | undefined> => {
  try {
    return await invoke('profile_quick_play_list', {
      profile
    });
  } catch (e: any) {}
};

export const profile_quick_play_remove = async (
  profile: string,
  quickPlay: QuickPlayInfo
): Promise<void | ProfileError> => {
  try {
    await invoke('profile_quick_play_remove', {
      profile,
      quickPlay
    });
  } catch (e: any) {
    return parseError(e);
  }
};

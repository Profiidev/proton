import { invoke } from '@tauri-apps/api/core';
import { type ProfileError, parseError } from './profile.svelte';

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
  } catch {
    return undefined;
  }
};

export const profile_quick_play_remove = async (
  profile: string,
  quickPlay: QuickPlayInfo
): Promise<undefined | ProfileError> => {
  try {
    await invoke('profile_quick_play_remove', {
      profile,
      quickPlay
    });
    return undefined;
  } catch (error: any) {
    return parseError(error);
  }
};

export const profile_quick_play_icon = async (
  profile: string,
  quickPlay: QuickPlayInfo
): Promise<string | undefined> => {
  try {
    return await invoke('profile_quick_play_icon', {
      profile,
      quickPlay
    });
  } catch {
    return undefined;
  }
};

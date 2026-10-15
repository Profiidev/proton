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

export interface ServerStatus {
  online: boolean;
  players_online: number;
  players_max: number;
}

const STATUS_TTL = 60_000;
const status_cache = new Map<
  string,
  { at: number; status: Promise<ServerStatus | undefined> }
>();

/*
 * Cached for a minute per address, so revisiting the multiplayer tab or list
 * refreshes do not hit the status api again. Not async, so the same promise
 * is returned while cached.
 */
// oxlint-disable-next-line promise-function-async
export const quick_play_server_status = (
  address: string
): Promise<ServerStatus | undefined> => {
  const cached = status_cache.get(address);
  if (cached && Date.now() - cached.at < STATUS_TTL) {
    return cached.status;
  }
  const status = invoke<ServerStatus | undefined>('quick_play_server_status', {
    address
  }).catch(() => undefined);
  status_cache.set(address, { at: Date.now(), status });
  return status;
};

import { invoke } from '@tauri-apps/api/core';
import { parseError } from './profile.svelte';

export const profile_runs_list = async (profile: string) => {
  try {
    return await invoke<string[]>('profile_runs_list', {
      profile
    });
  } catch (e: any) {}
};

export const profile_clear_logs = async (profile: string) => {
  try {
    await invoke('profile_clear_logs', {
      profile
    });
  } catch (e: any) {
    return parseError(e);
  }
};

export const profile_logs = async (
  profile: string,
  timestamp: string
): Promise<string[] | undefined> => {
  try {
    return await invoke('profile_logs', {
      profile,
      timestamp
    });
  } catch (e: any) {}
};

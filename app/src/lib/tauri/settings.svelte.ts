import { create_data_state, UpdateType } from '$lib/data_state.svelte';
import { invoke } from '@tauri-apps/api/core';
import { RequestError } from 'positron-components/backend';
import type { GameSettings, JvmSettings } from './profile.svelte';

export interface Settings {
  system_max_mem?: number; // in MB
  sidebar_width?: number;
  url?: URL;
  minecraft: MinecraftSettings;
}

export interface MinecraftSettings {
  show_snapshots: boolean;
  game_settings: GameSettings;
  jvm_settings: JvmSettings;
}

const settings_get_ = async (): Promise<Settings | undefined> => {
  try {
    return await invoke('settings_get');
  } catch (e) {}
};
export const settings_get = create_data_state(
  settings_get_,
  UpdateType.Settings
);

export const settings_set = async (settings: Settings) => {
  try {
    await invoke('settings_set', { settings });
  } catch (e) {
    return RequestError.Other;
  }
};

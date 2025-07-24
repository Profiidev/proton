import { create_data_state, UpdateType } from '$lib/data_state.svelte';
import { invoke } from '@tauri-apps/api/core';
import { RequestError } from 'positron-components/backend';

export interface Settings {
  sidebar_width?: number;
  url?: URL;
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

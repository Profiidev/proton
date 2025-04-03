import { create_data_state, UpdateType } from '$lib/data_state.svelte';
import { invoke } from '@tauri-apps/api/core';

const version_list_ = async (): Promise<string[] | undefined> => {
  try {
    return await invoke('version_list');
  } catch (e) {}
};
export const version_list = create_data_state(
  version_list_,
  UpdateType.Versions
);

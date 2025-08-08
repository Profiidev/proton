import { create_data_state, UpdateType } from '$lib/data_state.svelte';
import { invoke } from '@tauri-apps/api/core';
import { LoaderType } from './profile.svelte';

export const version_list = async (
  loader: LoaderType
): Promise<string[] | undefined> => {
  try {
    return await invoke('version_list', {
      loader
    });
  } catch (e) {}
};
export const vanilla_version_list = create_data_state(
  () => version_list(LoaderType.Vanilla),
  UpdateType.Versions
);

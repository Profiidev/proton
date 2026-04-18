import { UpdateType, create_data_state } from '$lib/data-state.svelte';
import { invoke } from '@tauri-apps/api/core';
import { LoaderType } from './profile.svelte';

export const version_list = async (
  loader: LoaderType
): Promise<string[] | undefined> => {
  try {
    return await invoke('version_list', {
      loader
    });
  } catch {
    return undefined;
  }
};
export const vanilla_version_list = create_data_state(
  async () => version_list(LoaderType.Vanilla),
  UpdateType.Versions
);

export const loader_version_list = async (
  loader: LoaderType,
  mcVersion: string
): Promise<string[] | undefined> => {
  try {
    return await invoke('loader_version_list', {
      loader,
      mcVersion
    });
  } catch {
    return undefined;
  }
};

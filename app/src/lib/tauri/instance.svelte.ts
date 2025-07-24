import { create_data_state, UpdateType } from '$lib/data_state.svelte';
import { invoke } from '@tauri-apps/api/core';
import type { LoaderType } from './profile.svelte';

export type InstanceInfo = {
  id: string;
  profile_name: string;
  profile_id: string;
  version: string;
  loader: LoaderType;
  loader_version?: string;
};

const instance_list_ = async (): Promise<InstanceInfo[] | undefined> => {
  try {
    return await invoke('instance_list');
  } catch (e) {}
};
export const instance_list = create_data_state(
  instance_list_,
  UpdateType.Instances
);

export const instance_logs = async (
  profile: string,
  id: string
): Promise<string[] | undefined> => {
  try {
    return await invoke('instance_logs', { profile, id });
  } catch (e) {}
};

export const instance_stop = async (
  profile: string,
  id: string
): Promise<void | undefined> => {
  try {
    await invoke('instance_stop', { profile, id });
  } catch (e) {}
};

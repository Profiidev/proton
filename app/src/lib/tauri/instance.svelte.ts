import { create_data_state, UpdateType } from '$lib/data_state.svelte';
import { invoke } from '@tauri-apps/api/core';

export type InstanceList = {
  [key: string]: InstanceInfo[];
};

export type InstanceInfo = {
  id: string;
};

const instance_list_ = async (): Promise<InstanceList | undefined> => {
  try {
    return await invoke('instance_list');
  } catch (e) {}
};
export const instance_list = create_data_state(
  instance_list_,
  UpdateType.Instances
);

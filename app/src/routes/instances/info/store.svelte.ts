import type { InstanceInfo } from '$lib/tauri/instance.svelte';

let instance = $state<InstanceInfo>();

export const setInstance = (i: InstanceInfo) => {
  instance = i;
};

export const getInstance = () => {
  return instance;
};

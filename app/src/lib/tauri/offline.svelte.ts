import { create_data_state, UpdateType } from '$lib/data_state.svelte';
import { invoke } from '@tauri-apps/api/core';
import { MANIFEST_REFRESH_ERROR_EVENT } from './events.svelte';
import { toast } from 'svelte-sonner';
import { browser } from '$app/environment';
import { listen } from '@tauri-apps/api/event';

const is_offline_ = async () => {
  try {
    return await invoke<boolean>('is_offline');
  } catch (e) {}
};
export const is_offline = create_data_state(is_offline_, UpdateType.Offline);

export const try_reconnect = async () => {
  try {
    return await invoke<boolean>('try_reconnect');
  } catch (e) {}
};

export const listen_manifest_refresh_error = async () => {
  if (!browser) return () => {};
  return await listen(MANIFEST_REFRESH_ERROR_EVENT, () => {
    toast.error(`Failed to refresh manifests.`);
  });
};

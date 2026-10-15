import { browser } from '$app/environment';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  type Download,
  type DownloadStatus,
  apply_status,
  clear_finished,
  new_download
} from '$lib/downloads';
import {
  VERSION_CHECK_STATUS_EVENT,
  type VersionCheckData
} from './events.svelte';

let list = $state.raw<Download[]>([]);

export const downloads = {
  get list() {
    return list;
  }
};

const update = (id: number, fn: (d: Download) => Download) => {
  list = list.map((d) => (d.id === id ? fn(d) : d));
};

export const download_start = (id: number, title: string) => {
  list = [...list, new_download(id, title, Date.now())];
};

/** Marks the download finished; returns the final status (canceled wins when requested) */
export const download_finish = (id: number, ok: boolean): DownloadStatus => {
  const canceled = list.find((d) => d.id === id)?.cancel_requested;
  let status: DownloadStatus = ok ? 'done' : 'error';
  if (canceled) {
    status = 'canceled';
  }
  update(id, (d) => ({ ...d, status }));
  return status;
};

export const download_cancel = async (id: number) => {
  update(id, (d) => ({ ...d, cancel_requested: true }));
  try {
    await invoke('profile_cancel_download', { id });
  } catch {
    // Empty
  }
};

export const downloads_clear_finished = () => {
  list = clear_finished(list);
};

if (browser) {
  const _ = listen<VersionCheckData>(VERSION_CHECK_STATUS_EVENT, (e) => {
    const { id, data } = e.payload;
    update(id, (d) => apply_status(d, data, Date.now()));
  });
}

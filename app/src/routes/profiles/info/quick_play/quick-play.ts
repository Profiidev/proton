import type { QuickPlayInfo, ServerStatus } from '$lib/tauri/quick-play.svelte';

export const players_label = (status?: ServerStatus) =>
  status &&
  (status.online
    ? `${status.players_online} / ${status.players_max} online`
    : 'Offline');

export const show_note = (
  worlds?: QuickPlayInfo[],
  servers?: QuickPlayInfo[]
) => !worlds?.length && !servers?.length;

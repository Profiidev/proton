import { relaunch } from '@tauri-apps/plugin-process';
import { check } from '@tauri-apps/plugin-updater';
import { toast } from 'svelte-sonner';
import { TOAST_DURATION } from './events.svelte';

let version = $state<string>();

export const checkForUpdates = async () => {
  try {
    const update = await check();
    if (update) {
      ({ version } = update);
      return update.version;
    }
  } catch {
    toast.error('Failed to check for updates');
  }
  return undefined;
};

export const getUpdateVersion = () => version;

let updaterToast: number | string | undefined = $state();

export const update = async () => {
  try {
    const updateData = await check();
    if (updateData) {
      let downloaded = 0;
      let contentLength = 0;
      // Alternatively we could also call update.download() and update.install() separately
      await updateData.downloadAndInstall((event) => {
        switch (event.event) {
          case 'Started': {
            contentLength = event.data.contentLength || 0;
            const id = Math.round(Math.random() * 1_000_000);
            updaterToast = toast.loading('Downloading update: 0%', {
              duration: TOAST_DURATION,
              id
            });
            break;
          }
          case 'Progress': {
            downloaded += event.data.chunkLength;
            updaterToast = toast.loading(
              `Downloading update: ${Math.round(
                (downloaded / contentLength) * 100
              )}%`,
              {
                duration: TOAST_DURATION,
                id: updaterToast
              }
            );
            break;
          }
          case 'Finished': {
            toast.dismiss(updaterToast);
            updaterToast = undefined;

            toast.success('Update downloaded successfully. Restarting ...');
            break;
          }
          default: {
            break;
          }
        }
      });

      if (updaterToast) {
        toast.dismiss(updaterToast);
        updaterToast = undefined;
      }

      await relaunch();
    }
  } catch {
    if (updaterToast) {
      toast.dismiss(updaterToast);
      updaterToast = undefined;
    }
    toast.error('Failed to download update');
  }
};

import { relaunch } from '@tauri-apps/plugin-process';
import { check } from '@tauri-apps/plugin-updater';
import { toast } from 'svelte-sonner';
import { TOAST_DURATION } from './events.svelte';

let version = $state<string>();

export const checkForUpdates = async () => {
  try {
    const update = await check();
    if (update) {
      version = update.version;
      return update.version;
    }
  } catch (e) {
    toast.error('Failed to check for updates');
    return null;
  }
};

export const getUpdateVersion = () => {
  return version;
};

let updaterToast: number | string | undefined = $state();

export const update = async () => {
  try {
    const update = await check();
    if (update) {
      let downloaded = 0;
      let contentLength = 0;
      // alternatively we could also call update.download() and update.install() separately
      await update.downloadAndInstall((event) => {
        switch (event.event) {
          case 'Started':
            contentLength = event.data.contentLength || 0;
            let id = Math.round(Math.random() * 1000000);
            updaterToast = toast.loading('Downloading update: 0%', {
              id,
              duration: TOAST_DURATION
            });
            break;
          case 'Progress':
            downloaded += event.data.chunkLength;
            updaterToast = toast.loading(
              `Downloading update: ${Math.round(
                (downloaded / contentLength) * 100
              )}%`,
              {
                id: updaterToast,
                duration: TOAST_DURATION
              }
            );
            break;
          case 'Finished':
            toast.dismiss(updaterToast);
            updaterToast = undefined;

            toast.success('Update downloaded successfully. Restarting ...');
            break;
        }
      });

      if (updaterToast) {
        toast.dismiss(updaterToast);
        updaterToast = undefined;
      }

      await relaunch();
    }
  } catch (e) {
    if (updaterToast) {
      toast.dismiss(updaterToast);
      updaterToast = undefined;
    }
    toast.error('Failed to download update');
  }
};

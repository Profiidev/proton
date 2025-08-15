import { create_data_state, UpdateType } from '$lib/data_state.svelte';
import { invoke } from '@tauri-apps/api/core';
import {
  TOAST_DURATION,
  VERSION_CHECK_STATUS_EVENT,
  type VersionCheckData,
  type VersionCheckStatus
} from './events.svelte';
import { listen } from '@tauri-apps/api/event';
import { browser } from '$app/environment';
import { toast } from 'positron-components/components/ui';
import type { QuickPlayInfo } from './quick_play.svelte';

export interface Profile {
  id: string;
  name: string;
  created_at: string;
  last_played?: string;
  last_played_non_quick_play?: string;
  favorite: boolean;
  version: string;
  loader: LoaderType;
  loader_version?: string;
  use_local_game: boolean;
  game?: GameSettings;
  use_local_jvm: boolean;
  jvm?: JvmSettings;
  use_local_dev: boolean;
  dev?: DevSettings;
}

export interface ProfileUpdate {
  id: string;
  name: string;
  version: string;
}

export interface GameSettings {
  width: number;
  height: number;
}

export interface JvmSettings {
  args: string[];
  env_vars: { [key: string]: string };
  mem_min: number;
  mem_max: number;
}

export interface DevSettings {
  show_console: boolean;
  keep_console_open: boolean;
}

export enum LoaderType {
  Vanilla = 'Vanilla',
  Fabric = 'Fabric',
  Quilt = 'Quilt',
  Forge = 'Forge',
  NeoForge = 'NeoForge'
}

export enum ProfileError {
  InvalidImage = 'InvalidImage',
  NotFound = 'NotFound',
  Other = 'Other'
}

export const parseError = (e: string) => {
  if (Object.values(ProfileError).includes(e as ProfileError)) {
    return e as ProfileError;
  } else {
    return ProfileError.Other;
  }
};

export const profile_create = async (data: {
  name: string;
  version: string;
  loader: LoaderType;
  loader_version?: string;
  icon?: Uint8Array;
}) => {
  try {
    await invoke('profile_create', data);
  } catch (e: any) {
    return parseError(e);
  }
};

export const profile_update = async (profile: ProfileUpdate) => {
  try {
    await invoke('profile_update', {
      profile
    });
  } catch (e: any) {
    return parseError(e);
  }
};

export const profile_get_icon = async (profile: string) => {
  try {
    return await invoke<string | undefined>('profile_get_icon', {
      profile
    });
  } catch (e: any) {}
};

export const profile_open_path = async (profile: string) => {
  try {
    await invoke<string>('profile_open_path', {
      profile
    });
  } catch (e: any) {
    return parseError(e);
  }
};

export const profile_update_icon = async (
  profile: string,
  icon: Uint8Array
) => {
  try {
    await invoke('profile_update_icon', {
      profile,
      icon
    });
  } catch (e: any) {
    return parseError(e);
  }
};

export const profile_remove = async (profile: string) => {
  try {
    await invoke('profile_remove', {
      profile
    });
  } catch (e: any) {
    return parseError(e);
  }
};

const profile_list_ = async (): Promise<Profile[] | undefined> => {
  try {
    return await invoke('profile_list');
  } catch (e) {}
};
export const profile_list = create_data_state(
  profile_list_,
  UpdateType.Profiles
);

export const profile_launch = async (
  profile: string,
  name: string,
  active?: string,
  quickPlay?: QuickPlayInfo
) => {
  if (active === undefined || active === '') {
    toast.warning('No active account set');
    return;
  }

  launch_repair(
    profile,
    'profile_launch',
    `Launching profile ${name}`,
    `Failed to launch profile ${name}`,
    quickPlay
  );
};

export const profile_repair = async (profile: string, name: string) => {
  launch_repair(
    profile,
    'profile_repair',
    `Repair of profile ${name} complete`,
    `Failed to repair profile ${name}`
  );
};

let check_toasts = new Map<number, string | number>();
let check_message = new Map<number, string>();
const launch_repair = async (
  profile: string,
  cmd: string,
  message: string,
  err: string,
  quickPlay?: QuickPlayInfo
) => {
  let id = Math.round(Math.random() * 1000000);
  try {
    check_toasts.set(
      id,
      toast.loading('Checking/Downloading version manifests', {
        duration: TOAST_DURATION
      })
    );
    check_message.set(id, message);
    await invoke(cmd, {
      profile,
      id,
      quickPlay
    });
  } catch (e: any) {
    toast.dismiss(check_toasts.get(id));
    check_message.delete(id);
    check_toasts.delete(id);

    toast.error(err);
  }
};

const get_message = (event: VersionCheckStatus): string | undefined => {
  if (typeof event === 'string') {
    switch (event) {
      case 'VersionManifestCheck':
        return 'Checking version manifest';
      case 'VersionManifestDownload':
        return 'Downloading version manifest';
      case 'AssetsManifestCheck':
        return 'Checking assets manifest';
      case 'AssetsManifestDownload':
        return 'Downloading assets manifest';
      case 'JavaManifestCheck':
        return 'Checking java manifest';
      case 'JavaManifestDownload':
        return 'Downloading java manifest';
      case 'ClientCheck':
        return 'Checking client jar';
      case 'ClientDownload':
        return 'Downloading client jar';
      case 'ModLoaderMeta':
        return 'Downloading ModLoader Version Meta';
      case 'ModLoaderPreprocess':
        return 'Preprocessing ModLoader files';
      case 'ModLoaderPreprocessDone':
        return 'ModLoader preprocessing done';
      case 'Done':
        return undefined; // No message for done
    }
  } else if ('AssetsCheck' in event) {
    let [done, total] = event.AssetsCheck;
    return `Checked ${done} of ${total} assets`;
  } else if ('AssetsDownload' in event) {
    let [done, total] = event.AssetsDownload;
    return `Downloaded ${done} of ${total} assets`;
  } else if ('JavaCheck' in event) {
    let [done, total] = event.JavaCheck;
    return `Checked ${done} of ${total} java files`;
  } else if ('JavaDownload' in event) {
    let [done, total] = event.JavaDownload;
    return `Downloaded ${done} of ${total} java files`;
  } else if ('NativeLibraryCheck' in event) {
    let [done, total] = event.NativeLibraryCheck;
    return `Checked ${done} of ${total} native libraries`;
  } else if ('NativeLibraryDownload' in event) {
    let [done, total] = event.NativeLibraryDownload;
    return `Downloaded ${done} of ${total} native libraries`;
  } else if ('LibraryCheck' in event) {
    let [done, total] = event.LibraryCheck;
    return `Checked ${done} of ${total} libraries`;
  } else if ('LibraryDownload' in event) {
    let [done, total] = event.LibraryDownload;
    return `Downloaded ${done} of ${total} libraries`;
  } else if ('ModLoaderFilesCheck' in event) {
    let [done, total] = event.ModLoaderFilesCheck;
    return `Checked ${done} of ${total} mod loader files`;
  } else if ('ModLoaderFilesDownload' in event) {
    let [done, total] = event.ModLoaderFilesDownload;
    return `Downloaded ${done} of ${total} mod loader files`;
  }
};

if (browser) {
  listen(VERSION_CHECK_STATUS_EVENT, (e) => {
    let event = e.payload as VersionCheckData;
    let id = check_toasts.get(event.id);
    if (id === undefined) return;

    let message = get_message(event.data);
    if (message) {
      check_toasts.set(
        event.id,
        toast.loading(message, {
          id,
          duration: TOAST_DURATION
        })
      );
    } else {
      toast.dismiss(id);
      check_toasts.delete(event.id);

      let message = check_message.get(event.id);
      check_message.delete(event.id);
      toast.success(message ?? '');
    }
  });
}

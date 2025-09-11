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
import DownloadNotificationCancel from '$lib/components/profile/DownloadNotificationCancel.svelte';
import type { ComponentProps } from 'svelte';
import DownloadNotification from '$lib/components/profile/DownloadNotification.svelte';
import { b_to_mb, debounce } from '$lib/util.svelte';

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
}

export interface ProfileUpdate {
  id: string;
  name: string;
  version: string;
  use_local_game: boolean;
  game?: GameSettings;
  use_local_jvm: boolean;
  jvm?: JvmSettings;
}

export interface GameSettings {
  use_custom: boolean;
  width: number;
  height: number;
}

export interface JvmSettings {
  args: string[];
  env_vars: { [key: string]: string };
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

export const ModdedLoaderType = {
  Fabric: LoaderType.Fabric,
  Quilt: LoaderType.Quilt,
  Forge: LoaderType.Forge,
  NeoForge: LoaderType.NeoForge
} as const;

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

let check_message = new Map<number, string>();
const cancel = (id: number) => (internal: any, props: any) => {
  return DownloadNotificationCancel(internal, { ...props, id });
};

const launch_repair = async (
  profile: string,
  cmd: string,
  message: string,
  err: string,
  quickPlay?: QuickPlayInfo
) => {
  let id = Math.round(Math.random() * 1000000);
  try {
    toast.loading('Checking/Downloading version manifests', {
      id,
      duration: TOAST_DURATION,
      cancel: cancel(id)
    });
    check_message.set(id, message);
    await invoke(cmd, {
      profile,
      id,
      quickPlay
    });
  } catch (e: any) {
    check_message.delete(id);

    toast.error(err, {
      id,
      duration: undefined,
      cancel: undefined
    });
  }
};

export const profile_cancel_download = async (id: number) => {
  try {
    await invoke('profile_cancel_download', { id });
  } catch (e) {}
};

const message_props = (
  info: [number, number],
  text: string,
  mib: boolean
): ComponentProps<typeof DownloadNotification> => {
  let [value, total] = info;
  return {
    text,
    total,
    value,
    convert: mib
      ? (value: number) => b_to_mb(value).toFixed(1) + 'MiB'
      : undefined
  };
};

const get_message = (
  event: VersionCheckStatus
): ComponentProps<typeof DownloadNotification> | string | undefined => {
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
      case 'ModLoaderMeta':
        return 'Downloading ModLoader version meta';
      case 'ModLoaderFilesDownloadInfo':
        return 'Downloading ModLoader file information';
      case 'ModLoaderPreprocess':
        return 'Running preprocessing of ModLoader';
      case 'ModLoaderPreprocessDone':
        return 'Preprocessing of ModLoader done';
      case 'Done':
        return undefined; // No message for done
    }
  } else if ('ClientDownload' in event) {
    return message_props(event.ClientDownload, 'Downloading client', true);
  } else if ('AssetsCheck' in event) {
    return message_props(event.AssetsCheck, 'Checking assets', false);
  } else if ('AssetsDownload' in event) {
    return message_props(event.AssetsDownload, 'Downloading assets', true);
  } else if ('JavaCheck' in event) {
    return message_props(event.JavaCheck, 'Checking java files', false);
  } else if ('JavaDownload' in event) {
    return message_props(event.JavaDownload, 'Downloading java files', true);
  } else if ('NativeLibraryCheck' in event) {
    return message_props(
      event.NativeLibraryCheck,
      'Checking native libraries',
      false
    );
  } else if ('NativeLibraryDownload' in event) {
    return message_props(
      event.NativeLibraryDownload,
      'Downloading native libraries',
      true
    );
  } else if ('LibraryCheck' in event) {
    return message_props(event.LibraryCheck, 'Checking libraries', false);
  } else if ('LibraryDownload' in event) {
    return message_props(event.LibraryDownload, 'Downloading libraries', true);
  } else if ('ModLoaderFilesCheck' in event) {
    return message_props(
      event.ModLoaderFilesCheck,
      'Checking mod loader files',
      false
    );
  } else if ('ModLoaderFilesDownload' in event) {
    return message_props(
      event.ModLoaderFilesDownload,
      'Downloading mod loader files',
      true
    );
  }
};

if (browser) {
  listen(VERSION_CHECK_STATUS_EVENT, (e) => {
    let event = e.payload as VersionCheckData;
    let id = event.id;
    if (id === undefined) return;

    let message = get_message(event.data);
    if (typeof message === 'object') {
      toast.loading(DownloadNotification, {
        id,
        componentProps: message
      });
    } else if (message) {
      toast.loading(message, {
        id
      });
    } else {
      let message = check_message.get(id);
      check_message.delete(id);
      toast.success(message ?? '', {
        id,
        duration: undefined,
        cancel: undefined
      });
    }
  });
}

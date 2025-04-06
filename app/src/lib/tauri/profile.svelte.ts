import { create_data_state, UpdateType } from '$lib/data_state.svelte';
import { invoke } from '@tauri-apps/api/core';
import { RequestError } from 'positron-components/backend';
import {
  TOAST_DURATION,
  VERSION_CHECK_STATUS_EVENT,
  type VersionCheckStatus
} from './events.svelte';
import { listen } from '@tauri-apps/api/event';
import { browser } from '$app/environment';
import { toast } from 'positron-components';

export interface Profile {
  id: string;
  name: string;
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
  Vanilla = 'Vanilla'
}

export enum ProfileError {
  InvalidImage = 'InvalidImage',
  NotFound = 'NotFound',
  Other = 'Other'
}

const parseError = (e: string) => {
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

export const profile_update = async (profile: Profile) => {
  try {
    await invoke('profile_update', {
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

let check_toast: string | number | undefined;
export const profile_launch = async (profile: string) => {
  try {
    check_toast = toast.loading('Checking/Downloading version manifests', {
      duration: TOAST_DURATION
    });
    await invoke('profile_launch', {
      profile
    });
  } catch (e: any) {
    return parseError(e);
  }
};

const get_message = (event: VersionCheckStatus): string | undefined => {
  if (typeof event === 'string') {
    switch (event) {
      case 'Client':
        return 'assets';
      case 'Done':
        return undefined;
    }
  } else if ('Manifest' in event) {
    return event.Manifest === 3
      ? 'client'
      : `version manifests ${event.Manifest}/3`;
  } else if ('Assets' in event) {
    let [done, total] = event.Assets;
    return done === total ? 'java files' : `assets ${done}/${total}`;
  } else if ('Java' in event) {
    let [done, total] = event.Java;
    return done === total ? 'native libraries' : `java files ${done}/${total}`;
  } else if ('NativeLibrary' in event) {
    let [done, total] = event.NativeLibrary;
    return done === total ? 'libraries' : `native libraries ${done}/${total}`;
  } else if ('Library' in event) {
    let [done, total] = event.Library;
    return `libraries ${done}/${total}`;
  }
};

if (browser) {
  listen(VERSION_CHECK_STATUS_EVENT, (e) => {
    if (check_toast === undefined) return;

    let event = e.payload as VersionCheckStatus;
    let message = get_message(event);

    if (message) {
      check_toast = toast.loading(`Downloading/Checking ${message}`, {
        id: check_toast,
        duration: TOAST_DURATION
      });
    } else {
      toast.dismiss(check_toast);
      check_toast = undefined;
      toast.success('Launching Minecraft');
    }
  });
}

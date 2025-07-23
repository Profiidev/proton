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
  console.log(data.icon);
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

export const profile_get_icon = async (profile: string) => {
  try {
    return await invoke<string | undefined>('profile_get_icon', {
      profile
    });
  } catch (e: any) {}
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

export const profile_launch = async (profile: string, name: string) => {
  launch_repair(
    profile,
    'profile_launch',
    `Launching profile ${name}`,
    `Failed to launch profile ${name}`
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
  err: string
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
      id
    });
  } catch (e: any) {
    toast.dismiss(id);
    check_message.delete(id);
    check_toasts.delete(id);

    toast.error(err);
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
    let event = e.payload as VersionCheckData;
    let id = check_toasts.get(event.id);
    if (id === undefined) return;

    let message = get_message(event.data);
    if (message) {
      check_toasts.set(
        event.id,
        toast.loading(`Downloading/Checking ${message}`, {
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

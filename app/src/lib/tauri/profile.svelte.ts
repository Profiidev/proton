import { UpdateType, create_data_state } from '$lib/data-state.svelte';
import { invoke } from '@tauri-apps/api/core';
import { toast } from '@profidev/pleiades/components/util/general';
import type { QuickPlayInfo } from './quick-play.svelte';
import { download_finish, download_start } from './downloads.svelte';

export interface Profile {
  id: string;
  name: string;
  created_at: string;
  last_played?: string;
  last_played_non_quick_play?: string;
  favorite: boolean;
  quick_play?: QuickPlayInfo[];
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
  env_vars: Record<string, string>;
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
  Forge: LoaderType.Forge,
  NeoForge: LoaderType.NeoForge,
  Quilt: LoaderType.Quilt
} as const;

export enum ProfileError {
  InvalidImage = 'InvalidImage',
  NotFound = 'NotFound',
  Other = 'Other'
}

export const parseError = (e: string) => {
  // oxlint-disable-next-line no-unsafe-type-assertion
  if (Object.values(ProfileError).includes(e as ProfileError)) {
    // oxlint-disable-next-line no-unsafe-type-assertion
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
  } catch (error: any) {
    return parseError(error);
  }
  return undefined;
};

export const profile_update = async (profile: ProfileUpdate) => {
  try {
    await invoke('profile_update', {
      profile
    });
  } catch (error: any) {
    return parseError(error);
  }
  return undefined;
};

export const profile_get_icon = async (profile: string) => {
  try {
    return await invoke<string | undefined>('profile_get_icon', {
      profile
    });
  } catch {
    return undefined;
  }
};

export const profile_open_path = async (profile: string) => {
  try {
    await invoke<string>('profile_open_path', {
      profile
    });
  } catch (error: any) {
    return parseError(error);
  }
  return undefined;
};

export const profile_update_icon = async (
  profile: string,
  icon: Uint8Array
) => {
  try {
    await invoke('profile_update_icon', {
      icon,
      profile
    });
  } catch (error: any) {
    return parseError(error);
  }
  return undefined;
};

export const profile_remove = async (profile: string) => {
  try {
    await invoke('profile_remove', {
      profile
    });
  } catch (error: any) {
    return parseError(error);
  }
  return undefined;
};

const profile_list_call = async (): Promise<Profile[] | undefined> => {
  try {
    return await invoke('profile_list');
  } catch {
    return undefined;
  }
};
export const profile_list = create_data_state(
  profile_list_call,
  UpdateType.Profiles
);

const profile_mods_count_call = async (): Promise<
  Record<string, number> | undefined
> => {
  try {
    return await invoke('profile_mods_count');
  } catch {
    return undefined;
  }
};
/** Number of mod jars per profile id */
export const profile_mods_count = create_data_state(
  profile_mods_count_call,
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

  const _ = launch_repair(
    profile,
    'profile_launch',
    `${name}: launch`,
    `Launching profile ${name}`,
    `Failed to launch profile ${name}`,
    quickPlay
  );
};

export const profile_repair = async (profile: string, name: string) => {
  const _ = launch_repair(
    profile,
    'profile_repair',
    `${name}: repair`,
    `Repair of profile ${name} complete`,
    `Failed to repair profile ${name}`
  );
};

const launch_repair = async (
  profile: string,
  cmd: string,
  title: string,
  message: string,
  err: string,
  quickPlay?: QuickPlayInfo
) => {
  const id = Math.round(Math.random() * 1_000_000);
  download_start(id, title);
  try {
    await invoke(cmd, {
      id,
      profile,
      quickPlay
    });
  } catch {
    download_finish(id, false);
    toast.error(err);
    return;
  }
  // The backend returns without error when the download was canceled
  if (download_finish(id, true) === 'canceled') {
    toast.warning('Check/Download canceled');
  } else {
    toast.success(message);
  }
};

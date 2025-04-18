import { create_data_state, UpdateType } from '$lib/data_state.svelte';
import { invoke } from '@tauri-apps/api/core';
import { RequestError } from 'positron-components/backend';

export type Accounts = { [key: string]: ProfileInfo | null };

export interface ProfileInfo {
  id: string;
  name: string;
  skins: Skin[];
  capes: Cape[];
}

export interface Skin {
  id: string;
  state: State;
  url: string;
  texture_key: string;
  variant: SkinVariant;
}

export interface Cape {
  id: string;
  state: State;
  url: string;
  alias: string;
}

export enum State {
  Active = 'ACTIVE',
  Inactive = 'INACTIVE'
}

export enum SkinVariant {
  Classic = 'CLASSIC',
  Slim = 'SLIM'
}

export interface SkinData {
  id: string;
  data: string;
  head: string;
  url: string;
}

export interface CapeData {
  id: string;
  data: string;
  url: string;
}

const account_list_ = async (): Promise<
  | {
      [key: string]: ProfileInfo | null;
    }
  | undefined
> => {
  try {
    return await invoke('account_list');
  } catch (e) {}
};

export const account_list = create_data_state(
  account_list_,
  UpdateType.Accounts
);

export const account_refresh = async () => {
  try {
    await invoke('account_refresh');
  } catch (e) {
    return RequestError.Other;
  }
};

export const account_refresh_one = async (id: string) => {
  try {
    await invoke('account_refresh_one', { id });
  } catch (e) {
    return RequestError.Other;
  }
};

export const account_login = async () => {
  try {
    await invoke('account_login');
  } catch (e) {
    return RequestError.Other;
  }
};

const account_get_active = async (): Promise<undefined | string> => {
  try {
    return await invoke('account_get_active');
  } catch (e) {}
};

export const account_active = create_data_state(
  account_get_active,
  UpdateType.AccountActive
);

export const account_set_active = async (id: string) => {
  try {
    await invoke('account_set_active', { id });
  } catch (e) {
    return RequestError.Other;
  }
};

export const account_remove = async (id: string) => {
  try {
    await invoke('account_remove', { id });
  } catch (e) {
    return RequestError.Other;
  }
};

export const account_get_skin = async (
  url: string
): Promise<undefined | SkinData> => {
  try {
    return await invoke('account_get_skin', { url });
  } catch (e) {}
};

export const account_get_cape = async (
  url: string
): Promise<undefined | CapeData> => {
  try {
    return await invoke('account_get_cape', { url });
  } catch (e) {}
};

export const account_add_skin = async (skin: Uint8Array) => {
  try {
    await invoke('account_add_skin', { skin });
  } catch (e) {
    return RequestError.Other;
  }
};

export const account_remove_skin = async (id: string) => {
  try {
    await invoke('account_remove_skin', { id });
  } catch (e) {
    return RequestError.Other;
  }
};

const account_list_skins_ = async (): Promise<SkinData[] | undefined> => {
  try {
    return await invoke('account_list_skins');
  } catch (e) {}
};
export const account_list_skins = create_data_state(
  account_list_skins_,
  UpdateType.AccountSkins
);

export const account_change_skin = async (id: string) => {
  try {
    await invoke('account_change_skin', { id });
  } catch (e) {
    return RequestError.Other;
  }
};

export const account_change_cape = async (id: string) => {
  try {
    await invoke('account_change_cape', { id });
  } catch (e) {
    return RequestError.Other;
  }
};

import { create_data_state } from "$lib/data_state.svelte";
import { invoke } from "@tauri-apps/api/core";

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
  Active = "ACTIVE",
  Inactive = "INACTIVE",
}

export enum SkinVariant {
  Classic = "CLASSIC",
  Slim = "SLIM",
}

export interface SkinData {
  data: string;
  head?: string;
}

const account_list_ = async (): Promise<
  | {
      [key: string]: ProfileInfo | null;
    }
  | undefined
> => {
  try {
    return await invoke("account_list");
  } catch (e) {}
};

export const account_list = create_data_state(account_list_);

export const account_refresh = async () => {
  try {
    await invoke("account_refresh");
    return true;
  } catch (e) {
    return false;
  }
};

export const account_refresh_one = async (id: string) => {
  try {
    await invoke("account_refresh_one", { id });
    return true;
  } catch (e) {
    return false;
  }
};

export const account_login = async () => {
  try {
    await invoke("account_login");
    return true;
  } catch (e) {
    return false;
  }
};

const account_get_active = async (): Promise<undefined | string> => {
  try {
    return await invoke("account_get_active");
  } catch (e) {}
};

export const account_active = create_data_state(account_get_active);

export const account_set_active = async (id: string) => {
  try {
    await invoke("account_set_active", { id });
    return true;
  } catch (e) {
    return false;
  }
};

export const account_remove = async (id: string) => {
  try {
    await invoke("account_remove", { id });
    return true;
  } catch (e) {
    return false;
  }
};

export const account_get_skin = async (
  url: string,
  head: boolean,
): Promise<undefined | SkinData> => {
  try {
    return await invoke("account_get_skin", { url, head });
  } catch (e) {}
};

export const account_clear_skins = async () => {
  try {
    await invoke("account_clear_skins");
    return true;
  } catch (e) {
    return false;
  }
};

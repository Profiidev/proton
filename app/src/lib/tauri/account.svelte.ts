import { invoke } from "@tauri-apps/api/core";

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

export const account_list = async (): Promise<
  | {
      [key: string]: ProfileInfo | null;
    }
  | undefined
> => {
  try {
    return await invoke("account_list");
  } catch (e) {}
};

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

import { browser } from '$app/environment';
import { listen } from '@tauri-apps/api/event';
import { tick } from 'svelte';

export enum UpdateType {
  //Accounts
  Accounts = 'Accounts',
  AccountActive = 'AccountActive',
  AccountSkins = 'AccountSkins',
  //Versions
  Versions = 'Versions',
  //Profiles
  Profiles = 'Profiles',
  ProfileLogs = 'ProfileLogs',
  ProfileQuickPlay = 'ProfileQuickPlay',
  //Instances
  Instances = 'Instances',
  InstanceLogs = 'InstanceLogs',
  //Settings
  Settings = 'Settings',
  //Offline
  Offline = 'Offline'
}

const updater_cbs = new Map<UpdateType, Map<string, () => void>>();
const UPDATE_EVENT = 'data-update';

if (browser) {
  const _ = listen(UPDATE_EVENT, (e) => {
    // oxlint-disable-next-line no-unsafe-type-assertion
    for (const cb of updater_cbs.get(e.payload as UpdateType)?.values() || []) {
      cb();
    }
  });
}

export const register_cb = (type: UpdateType, cb: () => void) => {
  const uuid = crypto.randomUUID();

  const existing = updater_cbs.get(type) || new Map();
  existing.set(uuid, cb);
  updater_cbs.set(type, existing);

  return uuid;
};

export const unregister_cb = (uuid: string, type: UpdateType) => {
  const type_cbs = updater_cbs.get(type);
  type_cbs?.delete(uuid);
};

export const create_data_state = <T>(
  update: () => Promise<T | undefined>,
  type: UpdateType
) => {
  let value: T | undefined = $state();

  let subscribers = 0;
  let uuid = '';

  return {
    update: async () => {
      const _ = update().then((v) => (value = v));
    },
    get value() {
      if ($effect.tracking()) {
        $effect(() => {
          if (subscribers === 0) {
            uuid = register_cb(type, async () => {
              value = await update();
            });
            const _ = update().then((v) => (value = v));
          }

          // oxlint-disable-next-line no-plusplus
          subscribers++;

          return () => {
            const _ = tick().then(() => {
              // oxlint-disable-next-line no-plusplus
              subscribers--;
              if (subscribers === 0) {
                unregister_cb(uuid, type);
              }
            });
          };
        });
      }

      return value;
    }
  };
};

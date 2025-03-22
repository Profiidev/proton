import { listen } from '@tauri-apps/api/event';
import { tick } from 'svelte';

export enum UpdateType {
  //accounts
  Accounts = 'Accounts',
  AccountActive = 'AccountActive',
  AccountSkins = 'AccountSkins',
  //versions
  Versions = 'Versions',
  //profiles
  Profiles = 'Profiles'
}

let updater_cbs = new Map<UpdateType, Map<string, () => void>>();
const UPDATE_EVENT = 'data-update';

listen(UPDATE_EVENT, (e) => {
  Array.from(updater_cbs.get(e.payload as UpdateType)?.values() || []).forEach(
    (cb) => cb()
  );
});

export const register_cb = (type: UpdateType, cb: () => void) => {
  let uuid = crypto.randomUUID().toString();

  let existing = updater_cbs.get(type) || new Map();
  existing.set(uuid, cb);
  updater_cbs.set(type, existing);

  return uuid;
};

export const unregister_cb = (uuid: string, type: UpdateType) => {
  let type_cbs = updater_cbs.get(type);
  type_cbs?.delete(uuid);
};

export const create_data_state = <T>(
  update: () => Promise<T | undefined>,
  type: UpdateType
) => {
  let value: T | undefined = $state();

  let subscribers = 0;
  let uuid: string;

  return {
    get value() {
      if ($effect.tracking()) {
        $effect(() => {
          if (subscribers === 0) {
            uuid = register_cb(type, async () => {
              value = await update();
            });
            update().then((v) => (value = v));
          }

          subscribers++;

          return () => {
            tick().then(() => {
              subscribers--;
              if (subscribers === 0) {
                unregister_cb(uuid, type);
              }
            });
          };
        });
      }

      return value;
    },
    update: async () => {
      update().then((v) => (value = v));
    }
  };
};

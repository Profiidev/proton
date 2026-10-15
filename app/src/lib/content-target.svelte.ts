const KEY = 'content-target';

const load = () => {
  try {
    return localStorage.getItem(KEY) ?? undefined;
  } catch {
    return undefined;
  }
};

let id: string | undefined = $state(load());

/** Profile Discover installs into, remembered across restarts */
export const content_target = {
  get id() {
    return id;
  },
  set id(value: string | undefined) {
    id = value;
    try {
      if (value === undefined) {
        localStorage.removeItem(KEY);
      } else {
        localStorage.setItem(KEY, value);
      }
    } catch {
      // Not persisted, still kept for this session
    }
  }
};

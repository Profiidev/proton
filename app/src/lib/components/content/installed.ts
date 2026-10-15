import { installed_state } from '$lib/tauri/content.svelte';
import type { ListKind } from './content-list';

const states = new Map<string, ReturnType<typeof installed_state>>();

/** One installed state per profile and kind, shared by the tab counts and the list */
export const installed = (profile: string, kind: ListKind) => {
  const key = `${profile}:${kind}`;
  let state = states.get(key);
  if (!state) {
    state = installed_state(profile, kind);
    states.set(key, state);
  }
  return state;
};

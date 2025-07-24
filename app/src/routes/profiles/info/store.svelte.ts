import type { Profile } from '$lib/tauri/profile.svelte';

let profile = $state<Profile>();

export const setProfile = (p: Profile) => {
  profile = p;
};

export const getProfile = () => {
  return profile;
};

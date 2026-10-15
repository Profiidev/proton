import type { Compat } from '$lib/content';
import type { VersionInfo } from '$lib/tauri/content.svelte';
import type { Profile } from '$lib/tauri/profile.svelte';

/** Version option text, e.g. `0.5.8 (release)` */
export const version_label = (v: VersionInfo) =>
  `${v.versionNumber} (${v.channel})`;

/** `id` when listed, else the newest release, else the newest version (lists are newest first) */
export const default_version = (versions: VersionInfo[], id?: string) =>
  versions.find((v) => v.id === id) ??
  versions.find((v) => v.channel === 'release') ??
  versions[0];

/** Profile tile note, e.g. `Fabric 1.21 · modpack, edits may be replaced on update` */
export const profile_note = (profile: Profile, compat: Compat) =>
  [
    `${profile.loader} ${profile.version}`,
    profile.modpack ? 'modpack, edits may be replaced on update' : undefined,
    compat.note
  ]
    .filter(Boolean)
    .join(' · ');

/** Whether the pack's recommended memory fits the available memory, both in MB */
// Leaves 2 GB for the OS, matching the backend when it applies the memory
export const memory_fits = (recommended: number, available?: number) =>
  available === undefined || recommended + 2048 <= available;

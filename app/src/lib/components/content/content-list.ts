import {
  ContentError,
  type ContentKind,
  type Installed,
  type Source
} from '$lib/tauri/content.svelte';

/** Kinds shown in the profile content tab */
export type ListKind = Exclude<ContentKind, 'modpack'>;

/** Singular noun of each kind, pluralize with `plural` */
export const KIND_WORD: Record<ListKind, string> = {
  mod: 'mod',
  resourcepack: 'resource pack',
  shader: 'shader'
};

export const SOURCE_LABEL: Record<Source, string> = {
  curseforge: 'CurseForge',
  modrinth: 'Modrinth'
};

/** Whether a dropped file can be added as `kind` */
export const accepts = (kind: ListKind, path: string) =>
  path.toLowerCase().endsWith(kind === 'mod' ? '.jar' : '.zip');

const sort_name = (r: Installed) => (r.name ?? r.fileName).toLowerCase();

/** Resource packs: enabled ones by priority, then disabled; others by name */
export const sort_rows = (kind: ListKind, rows: Installed[]) =>
  rows.toSorted((a, b) => {
    if (kind === 'resourcepack' && a.enabled !== b.enabled) {
      return Number(b.enabled) - Number(a.enabled);
    }
    if (kind === 'resourcepack' && a.enabled) {
      return (a.order ?? Infinity) - (b.order ?? Infinity);
    }
    return sort_name(a).localeCompare(sort_name(b));
  });

/** Toast text for a failed content command */
export const error_message = (error: ContentError) =>
  error === ContentError.Running
    ? 'Stop the game first'
    : 'Something went wrong, please try again';

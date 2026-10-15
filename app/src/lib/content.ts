import { latest_played, plural } from './profiles';
import type {
  ContentKind,
  Environment,
  Installed,
  ProjectSummary,
  SearchQuery,
  SearchSort,
  Source,
  UpdateInfo,
  Updates,
  VersionInfo
} from './tauri/content.svelte';
import { LoaderType, type Profile } from './tauri/profile.svelte';

/** Route segment of each kind, under `/discover` and the profile content tab */
export const KIND_ROUTE: Record<ContentKind, string> = {
  mod: 'mods',
  modpack: 'modpacks',
  resourcepack: 'resource-packs',
  shader: 'shaders'
};

export interface Compat {
  ok: boolean;
  note?: string;
}

/** Whether a mod for `loaders` (lowercase ids) runs on the profile's loader */
export const loader_compatible = (
  profile_loader: LoaderType,
  loaders: string[]
): Compat => {
  if (loaders.includes(profile_loader.toLowerCase())) {
    return { ok: true };
  }
  if (profile_loader === LoaderType.Quilt && loaders.includes('fabric')) {
    return { note: 'Fabric version, runs on Quilt', ok: true };
  }
  return { note: `Not available for ${profile_loader}`, ok: false };
};

/** Shaders need Iris, which runs on these loaders */
const SHADER_LOADERS = new Set([
  LoaderType.Fabric,
  LoaderType.Quilt,
  LoaderType.NeoForge
]);

/** Whether a project or one of its versions can be installed into `profile` */
export const profile_compat = (
  profile: Profile,
  kind: ContentKind,
  target: Pick<VersionInfo | ProjectSummary, 'gameVersions' | 'loaders'>
): Compat => {
  if (kind === 'modpack') {
    return { ok: true };
  }
  if (
    target.gameVersions.length > 0 &&
    !target.gameVersions.includes(profile.version)
  ) {
    return { note: `Not available for ${profile.version}`, ok: false };
  }
  if (kind === 'mod') {
    return loader_compatible(profile.loader, target.loaders);
  }
  if (kind === 'shader' && !SHADER_LOADERS.has(profile.loader)) {
    return { note: 'Shaders need Fabric, Quilt or NeoForge', ok: false };
  }
  return { ok: true };
};

/** Remembered target profile, else the latest played, else the first one */
export const pick_target = (profiles: Profile[], remembered?: string) =>
  profiles.find((p) => p.id === remembered) ??
  latest_played(profiles) ??
  profiles[0];

export interface Filters {
  query: string;
  sort: SearchSort;
  sources: Source[];
  /** Only show content for the target profile */
  compatible: boolean;
  loaders: string[];
  gameVersion?: string;
  categories: string[];
  environment?: Environment;
}

export const PAGE_SIZE = 20;

/** Search query for `kind`, sending only the filters that apply to it */
export const build_query = (
  kind: ContentKind,
  filters: Filters,
  target?: Profile,
  offset = 0
): SearchQuery => {
  const compatible =
    filters.compatible && target !== undefined && kind !== 'modpack';
  const has_loaders = kind === 'mod' || kind === 'modpack';
  return {
    categories: filters.categories,
    compatibleWith: compatible ? target.id : undefined,
    environment: has_loaders ? filters.environment : undefined,
    gameVersion: compatible ? undefined : filters.gameVersion,
    kind,
    limit: PAGE_SIZE,
    loaders: has_loaders && !compatible ? filters.loaders : [],
    offset,
    query: filters.query.trim(),
    sort: filters.sort,
    sources: filters.sources
  };
};

export type CardAction = 'install' | 'update' | 'installed';

/** Card button state against the target profile */
export const card_action = (
  summary: ProjectSummary,
  installed: Set<string>,
  updates: UpdateInfo[] = []
): CardAction => {
  if (updates.some((u) => u.projectId === summary.id)) {
    return 'update';
  }
  return installed.has(summary.id) ? 'installed' : 'install';
};

export type RowStatus = 'all' | 'enabled' | 'disabled' | 'updates';

/** Content list rows matching the search text and status, `updates` holds file names */
export const filter_rows = (
  rows: Installed[],
  text: string,
  status: RowStatus,
  updates = new Set<string>()
) => {
  const needle = text.trim().toLowerCase();
  return rows.filter(
    (row) =>
      (row.name ?? row.fileName).toLowerCase().includes(needle) &&
      (status === 'all' ||
        (status === 'enabled' && row.enabled) ||
        (status === 'disabled' && !row.enabled) ||
        (status === 'updates' && updates.has(row.fileName)))
  );
};

/** Installed entries that need `entry`, so removing it may break them */
export const blocking_dependents = (entry: Installed, all: Installed[]) =>
  all.filter(
    (other) =>
      other !== entry &&
      entry.dependencyOf.some(
        (dep) => dep === other.projectId || dep === other.fileName
      )
  );

/** Copy of `list` with the item at `from` moved to `to` */
export const move = <T>(list: T[], from: number, to: number) => {
  const next = [...list];
  next.splice(to, 0, ...next.splice(from, 1));
  return next;
};

const names = (list: string[]) =>
  list.length === 1
    ? list[0]
    : `${list.slice(0, -1).join(', ')} and ${list.at(-1)}`;

/** Home banner for available updates, undefined when there are none */
export const updates_banner = (per_profile: Updates, profiles: Profile[]) => {
  const affected = profiles.filter((p) => per_profile[p.id]?.length);
  if (affected.length === 0) {
    return undefined;
  }
  const all = affected.flatMap((p) => per_profile[p.id]);
  const word = all.every((u) => u.kind === 'mod') ? 'mod update' : 'update';
  const [first] = affected;
  return {
    count: all.length,
    description: `${names(affected.map((p) => p.name))} ${
      affected.length === 1 ? 'has' : 'have'
    } newer compatible versions.`,
    href: `/profiles/info/content/${KIND_ROUTE[per_profile[first.id][0].kind]}?id=${first.id}`,
    title: `${plural(all.length, word)} available`
  };
};

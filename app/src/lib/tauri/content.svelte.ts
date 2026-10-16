import { UpdateType, create_data_state } from '$lib/data-state.svelte';
import { invoke } from '@tauri-apps/api/core';
import { type LoaderType, run_job } from './profile.svelte';

export type Source = 'modrinth' | 'curseforge';
export type ContentKind = 'mod' | 'modpack' | 'resourcepack' | 'shader';
export type Channel = 'release' | 'beta' | 'alpha';
export type DependencyKind =
  | 'required'
  | 'optional'
  | 'incompatible'
  | 'embedded';
export type SearchSort = 'relevance' | 'downloads' | 'updated' | 'newest';
export type Environment = 'client' | 'server';

export interface ProjectSummary {
  source: Source;
  id: string;
  slug?: string;
  kind: ContentKind;
  name: string;
  author: string;
  description: string;
  iconUrl?: string;
  categories: string[];
  loaders: string[];
  gameVersions: string[];
  downloads: number;
  updatedAt: string;
  clientSide?: string;
  serverSide?: string;
  url: string;
}

export interface ProjectDetail {
  summary: ProjectSummary;
  /** Sanitized HTML */
  body: string;
  gallery: { url: string; title?: string }[];
  links: { source?: string; issues?: string; wiki?: string; discord?: string };
}

export interface FileInfo {
  fileName: string;
  /** Null for restricted CurseForge files */
  url?: string;
  sha1: string;
  sha512?: string;
  size: number;
  primary: boolean;
}

export interface Dependency {
  projectId: string;
  versionId?: string;
  kind: DependencyKind;
}

export interface VersionInfo {
  source: Source;
  projectId: string;
  id: string;
  name: string;
  versionNumber: string;
  channel: Channel;
  gameVersions: string[];
  loaders: string[];
  publishedAt: string;
  files: FileInfo[];
  dependencies: Dependency[];
}

export interface SearchQuery {
  kind: ContentKind;
  query: string;
  sources: Source[];
  loaders: string[];
  gameVersion?: string;
  categories: string[];
  environment?: Environment;
  sort: SearchSort;
  offset: number;
  limit: number;
  /** Profile id, the backend fills loaders and game version from it */
  compatibleWith?: string;
}

export interface SearchResult {
  hits: ProjectSummary[];
  total: number;
  warnings: string[];
}

export interface Category {
  name: string;
  /** Modrinth tag header, e.g. `categories` or `resolutions` */
  header: string;
  kind: ContentKind;
}

export interface Installed {
  kind: ContentKind;
  fileName: string;
  enabled: boolean;
  manual: boolean;
  fromPack: boolean;
  installedAt?: string;
  source?: Source;
  projectId?: string;
  versionId?: string;
  sha1: string;
  name?: string;
  iconUrl?: string;
  versionNumber?: string;
  dependencyOf: string[];
  /** Resource packs: 0 = highest priority */
  order?: number;
  /** Shaders */
  active?: boolean;
}

export interface PickRef {
  source: Source;
  projectId: string;
  versionId?: string;
}

export interface ManualFile {
  source: Source;
  projectId: string;
  fileId: string;
  fileName: string;
  sha1: string;
  size: number;
  projectUrl: string;
  /** Web page to open */
  downloadUrl: string;
}

export interface PlanItem {
  name: string;
  iconUrl?: string;
  kind: ContentKind;
  version: VersionInfo;
  reason: 'requested' | 'dependency';
  dependencyOf?: string;
  /** File name */
  replaces?: string;
}

export interface InstallPlan {
  profile: string;
  items: PlanItem[];
  alreadyInstalled: { projectId: string; name: string }[];
  optional: { source: Source; projectId: string; name: string }[];
  conflicts: { projectId: string; name: string; reason: string }[];
  manual: ManualFile[];
  totalBytes: number;
}

export interface UpdateInfo {
  profile: string;
  kind: ContentKind;
  fileName: string;
  source: Source;
  projectId: string;
  name: string;
  /** Installed version number */
  current: string;
  latest: VersionInfo;
}

/** A newer version of the pack a profile was installed from */
export interface PackUpdate {
  /** Installed version number, empty when the provider no longer lists it */
  current: string;
  latest: VersionInfo;
}

/** Update infos per profile id */
export type Updates = Record<string, UpdateInfo[]>;

export type PackSource =
  | { type: 'remote'; source: Source; projectId: string; versionId?: string }
  | { type: 'path'; path: string };

export interface PackFile {
  path: string;
  size: number;
  sha1: string;
  sha512?: string;
  url?: string;
}

export interface PackPlan {
  name: string;
  version: string;
  summary?: string;
  iconUrl?: string;
  source: PackSource;
  minecraft: string;
  loader: LoaderType;
  loaderVersion?: string;
  files: PackFile[];
  manual: ManualFile[];
  counts: {
    mods: number;
    resourcepacks: number;
    shaders: number;
    other: number;
  };
  recommendedMemoryMb?: number;
  totalBytes: number;
}

export enum ContentError {
  Incompatible = 'Incompatible',
  NeedsManualDownload = 'NeedsManualDownload',
  Network = 'Network',
  RateLimited = 'RateLimited',
  HashMismatch = 'HashMismatch',
  Unsupported = 'Unsupported',
  Running = 'Running',
  NotFound = 'NotFound',
  InvalidPack = 'InvalidPack',
  Other = 'Other'
}

export const parseContentError = (e: unknown) =>
  // oxlint-disable-next-line no-unsafe-type-assertion
  Object.values(ContentError).includes(e as ContentError)
    ? // oxlint-disable-next-line no-unsafe-type-assertion
      (e as ContentError)
    : ContentError.Other;

/** Runs a command that returns nothing, undefined on success */
const call = async (cmd: string, args: Record<string, unknown>) => {
  try {
    await invoke(cmd, args);
  } catch (error) {
    return parseContentError(error);
  }
  return undefined;
};

/** Runs a command whose value the UI needs, undefined on failure */
const get = async <T>(cmd: string, args: Record<string, unknown> = {}) => {
  try {
    return await invoke<T>(cmd, args);
  } catch {
    return undefined;
  }
};

/** Runs a command whose failure reason the UI shows, the error code on failure */
const get_or_error = async <T>(cmd: string, args: Record<string, unknown>) => {
  try {
    return await invoke<T>(cmd, args);
  } catch (error) {
    return parseContentError(error);
  }
};

export const content_sources = create_data_state(
  async () => get<Source[]>('content_sources'),
  UpdateType.Settings
);

export const content_updates = create_data_state(
  async () => get<Updates>('content_updates'),
  UpdateType.ContentUpdates
);

/** Pack updates per modpack profile id, only profiles with an update */
export const modpack_updates = create_data_state(
  async () => get<Record<string, PackUpdate>>('modpack_updates'),
  UpdateType.ContentUpdates
);

export const content_categories = async (kind: ContentKind) =>
  get<Category[]>('content_categories', { kind });

const CACHE_TTL = 60_000;
const cache = new Map<string, { at: number; value: Promise<any> }>();

/*
 * Cached for a minute per query, so going back to a page does not search
 * again. Not async, so the same promise is returned while cached.
 */
// oxlint-disable-next-line promise-function-async
const cached = <T>(
  cmd: string,
  args: Record<string, unknown>
): Promise<T | undefined> => {
  const key = `${cmd}:${JSON.stringify(args)}`;
  const hit = cache.get(key);
  if (hit && Date.now() - hit.at < CACHE_TTL) {
    return hit.value;
  }
  const value = invoke<T>(cmd, args).catch(() => {
    // Retry failed loads on the next call
    cache.delete(key);
    return undefined;
  });
  cache.set(key, { at: Date.now(), value });
  return value;
};

// oxlint-disable-next-line promise-function-async
export const content_search = (query: SearchQuery) =>
  cached<SearchResult>('content_search', { query });

// oxlint-disable-next-line promise-function-async
export const content_project = (source: Source, id: string) =>
  cached<ProjectDetail>('content_project', { id, source });

export const content_versions = async (
  source: Source,
  id: string,
  profile?: string
) => get<VersionInfo[]>('content_versions', { id, profile, source });

export const content_plan_install = async (profile: string, picks: PickRef[]) =>
  get_or_error<InstallPlan>('content_plan_install', { picks, profile });

export const content_install = async (plan: InstallPlan, name: string) =>
  run_job(
    'content_install',
    { plan },
    `${name}: install`,
    `Installed ${plan.items.length === 1 ? plan.items[0].name : `${plan.items.length} items`} in ${name}`,
    `Failed to install content in ${name}`
  );

const content_installed = async (profile: string, kind: ContentKind) =>
  get<Installed[]>('content_installed', { kind, profile });

/** Installed content of one kind, refreshed on content changes */
export const installed_state = (profile: string, kind: ContentKind) =>
  create_data_state(
    async () => content_installed(profile, kind),
    UpdateType.ProfileContent
  );

export const content_set_enabled = async (
  profile: string,
  kind: ContentKind,
  fileName: string,
  enabled: boolean
) => call('content_set_enabled', { enabled, fileName, kind, profile });

/** Resource packs, highest priority first */
export const content_set_order = async (profile: string, files: string[]) =>
  call('content_set_order', { files, profile });

export const content_set_active_shader = async (
  profile: string,
  fileName: string | null
) => call('content_set_active_shader', { fileName, profile });

export const content_remove = async (
  profile: string,
  kind: ContentKind,
  fileNames: string[]
) => call('content_remove', { fileNames, kind, profile });

export const content_add_file = async (profile: string, path: string) =>
  call('content_add_file', { path, profile });

export const content_adopt_file = async (
  profile: string,
  file: ManualFile,
  path: string
) => call('content_adopt_file', { file, path, profile });

/** File id -> path for the manual files found in the download folders */
export const content_find_manual = async (files: ManualFile[]) =>
  get_or_error<Record<string, string>>('content_find_manual', { files });

export const content_check_updates = async (profile?: string) =>
  get_or_error<Updates>('content_check_updates', { profile });

/** `fileNames` null updates everything */
export const content_update = async (
  profile: string,
  name: string,
  fileNames: string[] | null
) =>
  run_job(
    'content_update',
    { fileNames, profile },
    `${name}: update`,
    `Updated content in ${name}`,
    `Failed to update content in ${name}`
  );

export const modpack_plan = async (source: PackSource) =>
  get_or_error<PackPlan>('modpack_plan', { source });

/** Resolves to the new profile id, undefined on failure or cancel */
/** `manual` maps the restricted file ids to the downloaded files */
export const modpack_install = async (
  plan: PackPlan,
  name: string,
  manual: Record<string, string>
) =>
  run_job<string>(
    'modpack_install',
    { manual, name, plan },
    `${name}: install`,
    `Installed modpack ${name}`,
    `Failed to install modpack ${name}`
  );

/** Switches a modpack profile to the plan's version, user added content stays. True when done */
export const modpack_update = async (
  profile: string,
  name: string,
  plan: PackPlan,
  manual: Record<string, string>
) =>
  run_job<boolean>(
    'modpack_update',
    { manual, plan, profile },
    `${name}: update pack`,
    `Updated ${name} to ${plan.version}`,
    `Failed to update ${name}`
  );

export const profile_duplicate = async (profile: string, name: string) =>
  get_or_error<string>('profile_duplicate', { name, profile });

export const profile_export_mrpack = async (
  profile: string,
  name: string,
  path: string,
  include: string[]
) =>
  run_job(
    'profile_export_mrpack',
    { include, path, profile },
    `${name}: export`,
    `Exported ${name}`,
    `Failed to export ${name}`
  );

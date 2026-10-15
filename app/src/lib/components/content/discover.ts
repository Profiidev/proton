import type {
  Category,
  ContentKind,
  Installed,
  ProjectSummary
} from '$lib/tauri/content.svelte';

/** `world-generation` -> `World generation` */
export const tag_label = (tag: string) => {
  const text = tag.replaceAll('-', ' ');
  return text.charAt(0).toUpperCase() + text.slice(1);
};

const HEADER_LABEL: Record<string, string> = {
  features: 'Features',
  'performance impact': 'Performance cost',
  resolutions: 'Resolution'
};

const group_label = (kind: ContentKind, header: string) => {
  if (header === 'categories') {
    return kind === 'shader' ? 'Look' : 'Category';
  }
  return HEADER_LABEL[header] ?? tag_label(header);
};

/** Filter checkbox groups, `Category` first (`Look` on shaders) */
export const category_groups = (kind: ContentKind, categories: Category[]) => {
  const groups = new Map<string, string[]>();
  for (const c of categories) {
    groups.set(c.header, [...(groups.get(c.header) ?? []), c.name]);
  }
  return [...groups]
    .toSorted(
      ([a], [b]) => Number(b === 'categories') - Number(a === 'categories')
    )
    .map(([header, names]) => ({
      header,
      label: group_label(kind, header),
      names
    }));
};

/** `Client`, `Server`, `Client and server`, undefined when unknown */
export const runs_on = (
  p: Pick<ProjectSummary, 'clientSide' | 'serverSide'>
) => {
  const client = p.clientSide !== undefined && p.clientSide !== 'unsupported';
  const server = p.serverSide !== undefined && p.serverSide !== 'unsupported';
  if (client && server) {
    return 'Client and server';
  }
  if (client) {
    return 'Client';
  }
  return server ? 'Server' : undefined;
};

/** Iris and Sodium project ids on Modrinth and CurseForge */
const IRIS = new Set(['YL57xq9U', '455508']);
const SODIUM = new Set(['AANobbMI', '394468']);

/** Whether the installed mods already include Iris and Sodium */
export const shader_ready = (mods: Installed[]) =>
  mods.some((m) => IRIS.has(m.projectId ?? '')) &&
  mods.some((m) => SODIUM.has(m.projectId ?? ''));

/** Game versions newest first by the known version order, unknown ones last */
export const sort_versions = (versions: string[], known: string[] = []) => {
  const wanted = new Set(versions);
  const sorted = known.filter((v) => wanted.has(v));
  return [...sorted, ...versions.filter((v) => !sorted.includes(v))];
};

/** `a, b, c, +2 more` */
export const short_list = (list: string[], n = 3) =>
  list.length > n
    ? `${list.slice(0, n).join(', ')}, +${list.length - n} more`
    : list.join(', ');

const compact = new Intl.NumberFormat('en', { notation: 'compact' });
/** `2.1M` */
export const format_count = (n: number) => compact.format(n);

export const SOURCE_LABEL = { curseforge: 'CurseForge', modrinth: 'Modrinth' };

/** Project page link, `kind` is the content kind */
export const project_href = (
  p: Pick<ProjectSummary, 'kind' | 'source' | 'id'>
) =>
  `/discover/project?kind=${p.kind}&source=${p.source}&id=${encodeURIComponent(p.id)}`;

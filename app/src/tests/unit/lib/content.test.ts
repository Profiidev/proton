import { describe, expect, it } from 'vitest';
import {
  type Filters,
  blocking_dependents,
  build_query,
  card_action,
  filter_rows,
  loader_compatible,
  move,
  pick_target,
  profile_compat,
  updates_banner
} from '$lib/content';
import type {
  Installed,
  ProjectSummary,
  UpdateInfo
} from '$lib/tauri/content.svelte';
import { LoaderType, type Profile } from '$lib/tauri/profile.svelte';

const profile = (
  id: string,
  loader = LoaderType.Fabric,
  last_played?: string
): Profile => ({
  created_at: '2024-01-01T00:00:00Z',
  favorite: false,
  id,
  last_played,
  loader,
  name: `P ${id}`,
  use_local_game: false,
  use_local_jvm: false,
  version: '1.21'
});

const summary = (id: string): ProjectSummary => ({
  author: 'a',
  categories: [],
  description: '',
  downloads: 0,
  gameVersions: ['1.21'],
  id,
  kind: 'mod',
  loaders: ['fabric'],
  name: id,
  source: 'modrinth',
  updatedAt: '2024-01-01T00:00:00Z',
  url: ''
});

const row = (fileName: string, extra: Partial<Installed> = {}): Installed => ({
  dependencyOf: [],
  enabled: true,
  fileName,
  fromPack: false,
  kind: 'mod',
  manual: false,
  sha1: '',
  ...extra
});

const update = (
  profile_id: string,
  kind: UpdateInfo['kind'] = 'mod'
): UpdateInfo =>
  // oxlint-disable-next-line no-unsafe-type-assertion
  ({ fileName: 'f', kind, profile: profile_id, projectId: 'p' }) as UpdateInfo;

describe('loader_compatible', () => {
  it('matches the profile loader', () => {
    expect(loader_compatible(LoaderType.Fabric, ['fabric'])).toEqual({
      ok: true
    });
    expect(loader_compatible(LoaderType.NeoForge, ['neoforge']).ok).toBe(true);
  });
  it('lets Quilt run Fabric mods with a note', () => {
    expect(loader_compatible(LoaderType.Quilt, ['fabric'])).toEqual({
      note: 'Fabric version, runs on Quilt',
      ok: true
    });
  });
  it('rejects other loaders', () => {
    expect(loader_compatible(LoaderType.Forge, ['fabric']).ok).toBe(false);
    expect(loader_compatible(LoaderType.Fabric, ['quilt']).ok).toBe(false);
  });
});

describe('profile_compat', () => {
  const p = profile('x');
  it('checks game version', () => {
    expect(
      profile_compat(p, 'resourcepack', { gameVersions: ['1.20'], loaders: [] })
    ).toEqual({ note: 'Not available for 1.21', ok: false });
    expect(
      profile_compat(p, 'resourcepack', { gameVersions: [], loaders: [] }).ok
    ).toBe(true);
  });
  it('checks mod loaders', () => {
    expect(profile_compat(p, 'mod', summary('a')).ok).toBe(true);
    expect(
      profile_compat(profile('v', LoaderType.Vanilla), 'mod', summary('a')).ok
    ).toBe(false);
  });
  it('needs an Iris loader for shaders', () => {
    const s = { gameVersions: ['1.21'], loaders: ['iris'] };
    expect(profile_compat(p, 'shader', s).ok).toBe(true);
    expect(profile_compat(profile('f', LoaderType.Forge), 'shader', s).ok).toBe(
      false
    );
  });
  it('always allows modpacks', () => {
    expect(
      profile_compat(p, 'modpack', { gameVersions: ['1.8'], loaders: [] }).ok
    ).toBe(true);
  });
});

describe('pick_target', () => {
  const list = [
    profile('a', LoaderType.Fabric, '2024-01-01T00:00:00Z'),
    profile('b', LoaderType.Fabric, '2024-02-01T00:00:00Z'),
    profile('c')
  ];
  it('prefers the remembered profile', () => {
    expect(pick_target(list, 'c')?.id).toBe('c');
  });
  it('falls back to latest played, then the first', () => {
    expect(pick_target(list, 'gone')?.id).toBe('b');
    expect(pick_target([profile('z')])?.id).toBe('z');
    expect(pick_target([])).toBeUndefined();
  });
});

describe('build_query', () => {
  const filters: Filters = {
    categories: ['tech'],
    compatible: false,
    environment: 'client',
    gameVersion: '1.20',
    loaders: ['forge'],
    query: ' sodium ',
    sort: 'downloads',
    sources: ['modrinth']
  };
  it('sends mod filters', () => {
    expect(build_query('mod', filters, profile('x'), 40)).toEqual({
      categories: ['tech'],
      compatibleWith: undefined,
      environment: 'client',
      gameVersion: '1.20',
      kind: 'mod',
      limit: 20,
      loaders: ['forge'],
      offset: 40,
      query: 'sodium',
      sort: 'downloads',
      sources: ['modrinth']
    });
  });
  it('lets the backend fill loader and version when compatible', () => {
    const q = build_query(
      'mod',
      { ...filters, compatible: true },
      profile('x')
    );
    expect(q).toMatchObject({ compatibleWith: 'x', loaders: [] });
    expect(q.gameVersion).toBeUndefined();
  });
  it('drops loaders and environment for packs and shaders', () => {
    const q = build_query('resourcepack', filters);
    expect(q.loaders).toEqual([]);
    expect(q.environment).toBeUndefined();
  });
  it('never targets a profile for modpacks', () => {
    const q = build_query(
      'modpack',
      { ...filters, compatible: true },
      profile('x')
    );
    expect(q.compatibleWith).toBeUndefined();
    expect(q.loaders).toEqual(['forge']);
  });
});

describe('card_action', () => {
  it('picks install, installed or update', () => {
    const installed = new Set(['p']);
    expect(card_action(summary('n'), installed)).toBe('install');
    expect(card_action(summary('p'), installed)).toBe('installed');
    expect(card_action(summary('p'), installed, [update('x')])).toBe('update');
  });
});

describe('filter_rows', () => {
  const rows = [
    row('sodium.jar', { name: 'Sodium' }),
    row('lithium.jar', { enabled: false }),
    row('iris.jar', { name: 'Iris' })
  ];
  it('matches name or file name', () => {
    expect(filter_rows(rows, 'SOD', 'all')).toEqual([rows[0]]);
    expect(filter_rows(rows, 'lith', 'all')).toEqual([rows[1]]);
  });
  it('filters by status', () => {
    expect(filter_rows(rows, '', 'disabled')).toEqual([rows[1]]);
    expect(filter_rows(rows, '', 'enabled')).toHaveLength(2);
    expect(filter_rows(rows, '', 'updates', new Set(['iris.jar']))).toEqual([
      rows[2]
    ]);
  });
});

describe('blocking_dependents', () => {
  it('finds entries that need the removed one', () => {
    const fabric = row('fabric-api.jar', { dependencyOf: ['sodium'] });
    const sodium = row('sodium.jar', { projectId: 'sodium' });
    const other = row('other.jar', { projectId: 'other' });
    expect(blocking_dependents(fabric, [fabric, sodium, other])).toEqual([
      sodium
    ]);
    expect(blocking_dependents(sodium, [fabric, sodium, other])).toEqual([]);
  });
});

describe('move', () => {
  it('moves without mutating', () => {
    const list = ['a', 'b', 'c', 'd'];
    expect(move(list, 0, 2)).toEqual(['b', 'c', 'a', 'd']);
    expect(move(list, 3, 0)).toEqual(['d', 'a', 'b', 'c']);
    expect(list).toEqual(['a', 'b', 'c', 'd']);
  });
});

describe('updates_banner', () => {
  const list = [profile('a'), profile('b'), profile('c')];
  it('hides without updates', () => {
    expect(updates_banner({}, list)).toBeUndefined();
    expect(updates_banner({ a: [] }, list)).toBeUndefined();
  });
  it('counts and names affected profiles', () => {
    expect(
      updates_banner(
        {
          b: [update('b'), update('b')],
          c: [update('c')],
          gone: [update('g')]
        },
        list
      )
    ).toEqual({
      count: 3,
      description: 'P b and P c have newer compatible versions.',
      href: '/profiles/info/content/mods?id=b',
      title: '3 mod updates available'
    });
  });
  it('uses a generic word for mixed kinds', () => {
    expect(updates_banner({ a: [update('a', 'shader')] }, list)).toMatchObject({
      description: 'P a has newer compatible versions.',
      href: '/profiles/info/content/shaders?id=a',
      title: '1 update available'
    });
  });
});

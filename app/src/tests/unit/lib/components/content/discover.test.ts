import { describe, expect, it } from 'vitest';
import {
  category_groups,
  format_count,
  runs_on,
  shader_ready,
  short_list,
  sort_versions,
  tag_label
} from '$lib/components/content/discover';
import type { Installed } from '$lib/tauri/content.svelte';

const mod = (projectId: string): Installed => ({
  dependencyOf: [],
  enabled: true,
  fileName: `${projectId}.jar`,
  fromPack: false,
  kind: 'mod',
  manual: false,
  projectId,
  sha1: ''
});

describe('discover helpers', () => {
  it('labels tags', () => {
    expect(tag_label('world-generation')).toBe('World generation');
  });

  it('groups categories with categories first', () => {
    const groups = category_groups('shader', [
      { header: 'performance impact', kind: 'shader', name: 'low' },
      { header: 'categories', kind: 'shader', name: 'cartoon' },
      { header: 'performance impact', kind: 'shader', name: 'high' }
    ]);
    expect(groups).toEqual([
      { header: 'categories', label: 'Look', names: ['cartoon'] },
      {
        header: 'performance impact',
        label: 'Performance cost',
        names: ['low', 'high']
      }
    ]);
    expect(
      category_groups('mod', [
        { header: 'categories', kind: 'mod', name: 'x' }
      ])[0].label
    ).toBe('Category');
  });

  it('describes where a project runs', () => {
    expect(runs_on({ clientSide: 'required', serverSide: 'optional' })).toBe(
      'Client and server'
    );
    expect(runs_on({ clientSide: 'required', serverSide: 'unsupported' })).toBe(
      'Client'
    );
    expect(runs_on({})).toBeUndefined();
  });

  it('needs both Iris and Sodium for shaders', () => {
    expect(shader_ready([mod('YL57xq9U'), mod('AANobbMI')])).toBe(true);
    expect(shader_ready([mod('455508')])).toBe(false);
  });

  it('sorts versions by the known order', () => {
    expect(
      sort_versions(['1.20', 'snap', '1.21'], ['1.21', '1.20.1', '1.20'])
    ).toEqual(['1.21', '1.20', 'snap']);
  });

  it('shortens lists and counts', () => {
    expect(short_list(['a', 'b', 'c', 'd', 'e'])).toBe('a, b, c, +2 more');
    expect(short_list(['a', 'b'])).toBe('a, b');
    expect(format_count(2_100_000)).toBe('2.1M');
  });
});

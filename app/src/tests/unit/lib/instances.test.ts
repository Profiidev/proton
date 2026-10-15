import { describe, expect, it } from 'vitest';
import {
  format_duration,
  format_gb,
  group_by_profile,
  instance_number,
  running_label
} from '$lib/instances';
import type { InstanceInfo } from '$lib/tauri/instance.svelte';
import { LoaderType } from '$lib/tauri/profile.svelte';

const inst = (id: string, profile_id: string, launched_at: string) =>
  ({
    id,
    launched_at,
    loader: LoaderType.Vanilla,
    profile_id,
    profile_name: profile_id,
    version: '1.21'
  }) satisfies InstanceInfo;

const list = [
  inst('c', 'p1', '2026-01-01T12:00:00Z'),
  inst('a', 'p1', '2026-01-01T10:00:00Z'),
  inst('b', 'p2', '2026-01-01T11:00:00Z')
];

describe('instances helpers', () => {
  it('groups by profile ordered by launch time', () => {
    const groups = group_by_profile(list);
    expect([...groups.keys()]).toEqual(['p1', 'p2']);
    expect(groups.get('p1')?.map((i) => i.id)).toEqual(['a', 'c']);
  });

  it('numbers instances per profile', () => {
    expect(instance_number(list, list[0])).toBe(2);
    expect(instance_number(list, list[1])).toBe(1);
    expect(instance_number(list, list[2])).toBe(1);
  });

  it('labels running counts', () => {
    expect(running_label(1)).toBe('Running');
    expect(running_label(3)).toBe('3 running');
  });

  it('formats memory and durations', () => {
    expect(format_gb(3.1 * 1024 ** 3)).toBe('3.1');
    expect(format_duration(30_000)).toBe('<1m');
    expect(format_duration(5 * 60_000)).toBe('5m');
    expect(format_duration(72 * 60_000)).toBe('1h 12m');
  });
});

import { describe, expect, it } from 'vitest';
import {
  accepts,
  error_message,
  sort_rows
} from '$lib/components/content/content-list';
import { ContentError, type Installed } from '$lib/tauri/content.svelte';

const row = (fileName: string, extra: Partial<Installed> = {}): Installed => ({
  dependencyOf: [],
  enabled: true,
  fileName,
  fromPack: false,
  kind: 'resourcepack',
  manual: false,
  sha1: '',
  ...extra
});

describe('accepts', () => {
  it('takes jars for mods and zips for packs and shaders', () => {
    expect(accepts('mod', '/a/Sodium.JAR')).toBe(true);
    expect(accepts('mod', '/a/pack.zip')).toBe(false);
    expect(accepts('resourcepack', '/a/pack.zip')).toBe(true);
    expect(accepts('shader', '/a/mod.jar')).toBe(false);
  });
});

describe('sort_rows', () => {
  it('puts enabled packs first by priority, then disabled', () => {
    const rows = [
      row('off.zip', { enabled: false }),
      row('low.zip', { order: 1 }),
      row('high.zip', { order: 0 })
    ];
    expect(sort_rows('resourcepack', rows).map((r) => r.fileName)).toEqual([
      'high.zip',
      'low.zip',
      'off.zip'
    ]);
  });

  it('sorts other kinds by name, falling back to the file name', () => {
    const rows = [
      row('b.jar', { enabled: false }),
      row('x.jar', { name: 'A' })
    ];
    expect(sort_rows('mod', rows).map((r) => r.fileName)).toEqual([
      'x.jar',
      'b.jar'
    ]);
  });
});

describe('error_message', () => {
  it('asks to stop the game when it is running', () => {
    expect(error_message(ContentError.Running)).toBe('Stop the game first');
    expect(error_message(ContentError.Network)).not.toBe('Stop the game first');
  });
});

import { describe, expect, it } from 'vitest';
import {
  default_version,
  memory_fits,
  profile_note,
  version_label
} from '$lib/components/content/install';
import type { VersionInfo } from '$lib/tauri/content.svelte';
import { LoaderType, type Profile } from '$lib/tauri/profile.svelte';

const version = (id: string, channel: VersionInfo['channel']) =>
  // oxlint-disable-next-line no-unsafe-type-assertion
  ({ channel, id, versionNumber: `v${id}` }) as VersionInfo;

const profile = (extra: Partial<Profile> = {}) =>
  // oxlint-disable-next-line no-unsafe-type-assertion
  ({ loader: LoaderType.Fabric, version: '1.21', ...extra }) as Profile;

describe('install', () => {
  it('version_label', () => {
    expect(version_label(version('1', 'beta'))).toBe('v1 (beta)');
  });

  it('default_version', () => {
    const list = [version('3', 'beta'), version('2', 'release')];
    expect(default_version(list)?.id).toBe('2');
    expect(default_version(list, '3')?.id).toBe('3');
    expect(default_version(list, 'missing')?.id).toBe('2');
    expect(default_version([version('1', 'alpha')])?.id).toBe('1');
    expect(default_version([])).toBeUndefined();
  });

  it('profile_note', () => {
    expect(profile_note(profile(), { ok: true })).toBe('Fabric 1.21');
    expect(
      profile_note(
        profile({
          modpack: { projectId: 'p', source: 'modrinth', versionId: 'v' }
        }),
        { note: 'Not here', ok: false }
      )
    ).toBe('Fabric 1.21 · modpack, edits may be replaced on update · Not here');
  });

  it('memory_fits', () => {
    expect(memory_fits(4096, 8192)).toBe(true);
    expect(memory_fits(7168, 8192)).toBe(false);
    expect(memory_fits(8192, 4096)).toBe(false);
    expect(memory_fits(8192)).toBe(true);
  });
});

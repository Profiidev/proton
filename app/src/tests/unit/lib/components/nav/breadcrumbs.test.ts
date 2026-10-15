import { describe, expect, it } from 'vitest';
import { breadcrumbs } from '$lib/components/nav/breadcrumbs';

const profiles = [{ id: 'x', name: 'Survival' }];
const instances = [{ id: 'i1', profile_name: 'Modded' }];

describe('breadcrumbs', () => {
  it('shows profile name for profile subpages', () => {
    expect(
      breadcrumbs('/profiles/info/settings/java', '?id=x', profiles)
    ).toEqual([
      { href: '/profiles', label: 'Profiles' },
      { href: '/profiles/info?id=x', label: 'Survival' },
      { href: '/profiles/info/settings?id=x', label: 'Settings' },
      { href: '/profiles/info/settings/java?id=x', label: 'Java' }
    ]);
  });

  it('title-cases underscored segments', () => {
    expect(
      breadcrumbs('/profiles/info/quick_play', '?id=x', profiles).at(-1)
    ).toEqual({ href: '/profiles/info/quick_play?id=x', label: 'Quick Play' });
  });

  it("shows the instance's profile name", () => {
    expect(
      breadcrumbs('/instances/info/logs', '?id=i1', [], instances).map(
        (c) => c.label
      )
    ).toEqual(['Instances', 'Modded', 'Logs']);
  });

  it('falls back when the entity is unknown', () => {
    expect(breadcrumbs('/profiles/info', '?id=y', profiles)[1].label).toBe(
      'Profile'
    );
  });

  it('handles top-level and root pages', () => {
    expect(breadcrumbs('/settings/java', '')).toEqual([
      { href: '/settings', label: 'Settings' },
      { href: '/settings/java', label: 'Java' }
    ]);
    expect(breadcrumbs('/', '')).toEqual([]);
  });
});

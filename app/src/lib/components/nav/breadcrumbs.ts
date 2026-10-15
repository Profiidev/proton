import { KIND_ROUTE } from '$lib/content';
import type { ContentKind } from '$lib/tauri/content.svelte';

export interface Crumb {
  label: string;
  href: string;
}

const title = (part: string) =>
  part
    .split(/[_-]/)
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(' ');

/**
 * Breadcrumb trail for a route: section, then profile name (for `info` pages), then subpages.
 * Links below an `info` segment keep the query (`?id=...`) so they resolve to the same entity.
 */
export const breadcrumbs = (
  pathname: string,
  search: string,
  profiles: { id: string; name: string }[] = [],
  instances: { id: string; profile_name: string }[] = [],
  projects: Record<string, string> = {}
): Crumb[] => {
  const parts = pathname.split('/').filter(Boolean);
  const params = new URLSearchParams(search);
  const id = params.get('id');
  if (parts[0] === 'discover' && parts[1] === 'project') {
    // oxlint-disable-next-line no-unsafe-type-assertion
    const kind = KIND_ROUTE[params.get('kind') as ContentKind];
    return [
      { href: '/discover', label: 'Discover' },
      ...(kind ? [{ href: `/discover/${kind}`, label: title(kind) }] : []),
      {
        href: pathname + search,
        label: (id && projects[id]) || 'Project'
      }
    ];
  }
  return parts.map((part, i) => {
    let label = title(part);
    if (part === 'info' && i === 1) {
      if (parts[0] === 'profiles') {
        label = profiles.find((p) => p.id === id)?.name ?? 'Profile';
      } else if (parts[0] === 'instances') {
        label =
          instances.find((inst) => inst.id === id)?.profile_name ?? 'Instance';
      }
    }
    const path = `/${parts.slice(0, i + 1).join('/')}`;
    return {
      href: i >= 1 && parts[1] === 'info' ? path + search : path,
      label
    };
  });
};

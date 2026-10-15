export interface Crumb {
  label: string;
  href: string;
}

const title = (part: string) =>
  part
    .split('_')
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
  instances: { id: string; profile_name: string }[] = []
): Crumb[] => {
  const parts = pathname.split('/').filter(Boolean);
  const id = new URLSearchParams(search).get('id');
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

import type { InstanceInfo } from './tauri/instance.svelte';

/** Instances grouped by profile id, each group ordered oldest first */
export const group_by_profile = (instances: InstanceInfo[]) => {
  const groups = new Map<string, InstanceInfo[]>();
  for (const instance of instances.toSorted((a, b) =>
    a.launched_at.localeCompare(b.launched_at)
  )) {
    const group = groups.get(instance.profile_id) ?? [];
    group.push(instance);
    groups.set(instance.profile_id, group);
  }
  return groups;
};

/** 1-based position of the instance among the running instances of its profile */
export const instance_number = (
  instances: InstanceInfo[],
  instance: InstanceInfo
) =>
  (group_by_profile(instances)
    .get(instance.profile_id)
    ?.findIndex((i) => i.id === instance.id) ?? 0) + 1;

export const running_label = (count: number) =>
  count > 1 ? `${count} running` : 'Running';

export const format_gb = (bytes: number) =>
  (bytes / 1024 / 1024 / 1024).toFixed(1);

/** Compact duration like `1h 12m`, `5m` or `<1m` */
export const format_duration = (ms: number) => {
  const minutes = Math.floor(ms / 60_000);
  if (minutes < 1) {
    return '<1m';
  }
  const hours = Math.floor(minutes / 60);
  return hours > 0 ? `${hours}h ${minutes % 60}m` : `${minutes}m`;
};

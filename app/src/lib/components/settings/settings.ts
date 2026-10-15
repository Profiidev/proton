const gb = (mb: number) => (mb / 1024).toFixed(1);

/** Profile memory label, e.g. `6.0 GB of 13.7 GB` */
export const memory_of_total = (mb: number, total_mb: number) =>
  `${gb(mb)} GB of ${gb(total_mb)} GB`;

/** Global memory label, e.g. `13.7 GB (14016 MB)` */
export const memory_with_mb = (mb: number) => `${gb(mb)} GB (${mb} MB)`;

/** Splits a java argument string on whitespace */
export const parse_args = (args: string) =>
  args.trim().split(/\s+/).filter(Boolean);

/** Env vars sorted by key */
export const sorted_env = (env: Record<string, string>) =>
  Object.entries(env).toSorted(([a], [b]) => a.localeCompare(b));

/** Returns the new env map or an error message */
export const add_env_var = (
  env: Record<string, string>,
  raw_key: string,
  raw_value: string
): Record<string, string> | string => {
  const key = raw_key.trim();
  const value = raw_value.trim();
  if (!key || !value) {
    return 'Both key and value must be provided';
  }
  if (key.includes('=')) {
    return 'Environment variable key cannot contain "="';
  }
  if (key in env) {
    return `Environment variable "${key}" already exists`;
  }
  return { ...env, [key]: value };
};

export const remove_env_var = (env: Record<string, string>, key: string) => {
  const { [key]: _, ...rest } = env;
  return rest;
};

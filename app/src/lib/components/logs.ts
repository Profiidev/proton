export interface LogEntry {
  time: string;
  level: string;
  location: string;
  message: string;
  subLocation?: string;
  /** Unparsable line (e.g. stack trace) belonging to the previous entry, `level` is inherited */
  continuation?: boolean;
}

export const LOG_LEVELS = ['INFO', 'WARN', 'ERROR', 'DEBUG'];

export const LEVEL_COLORS: Record<string, string> = {
  DEBUG: 'text-blue-500',
  ERROR: 'text-red-500',
  INFO: 'text-green-500',
  WARN: 'text-yellow-500'
};

/** Console toggle modes: All = everything, Warnings = WARN and ERROR, Errors = ERROR only */
export const CONSOLE_LEVELS: Record<string, string[]> = {
  all: [],
  err: ['ERROR'],
  warn: ['WARN', 'ERROR']
};

/** Parses `[time] [location/LEVEL] ([subLocation]): message`, unparsable lines become a bare message */
export const parse_line = (line: string): LogEntry => {
  const groups =
    /^\[(?<time>.+?)\] \[(?<location>.+?)\/(?<level>.+?)\](?: \[(?<subLocation>.+?)\])?: (?<message>.+)$/.exec(
      line
    )?.groups;
  if (!groups) {
    return { level: '', location: '', message: line, time: '' };
  }
  const { level, location, message, subLocation, time } = groups;
  return { level, location, message, subLocation, time };
};

/**
 * Keeps entries matching any of `levels` (empty = all levels) whose
 * location, sub location or message contains `search` (case insensitive).
 */
/** Parses all lines, continuation lines inherit the level of the previous parsed line */
export const parse_logs = (lines: string[]) => {
  let level = '';
  return lines.map((line) => {
    const entry = parse_line(line);
    if (entry.level) {
      ({ level } = entry);
      return entry;
    }
    return { ...entry, continuation: true, level };
  });
};

export const filter_logs = (
  entries: LogEntry[],
  search: string,
  levels: string[]
) => {
  const query = search.trim().toLowerCase();
  return entries.filter(
    (e) =>
      (levels.length === 0 || levels.includes(e.level)) &&
      (!query ||
        [e.location, e.subLocation, e.message].some((s) =>
          s?.toLowerCase().includes(query)
        ))
  );
};

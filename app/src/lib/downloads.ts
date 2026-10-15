import type { VersionCheckStatus } from './tauri/events.svelte';

export type DownloadStatus = 'running' | 'done' | 'error' | 'canceled';

export interface Download {
  id: number;
  title: string;
  status: DownloadStatus;
  started_at: number;
  /** Current stage text, e.g. `Downloading assets` */
  stage: string;
  value?: number;
  total?: number;
  /** Stage counts bytes (shown in MiB, has a speed) */
  bytes: boolean;
  /** Counted thing of a count stage, e.g. `files` */
  unit?: string;
  /** Smoothed bytes/s of the current byte stage */
  speed?: number;
  /** Time of the last byte-stage value, for speed deltas */
  updated_at?: number;
  cancel_requested?: boolean;
}

interface Stage {
  stage: string;
  progress?: [number, number];
  bytes?: boolean;
  unit?: string;
}

const SIMPLE_STAGES: Record<string, string> = {
  AssetsManifestCheck: 'Checking assets manifest',
  AssetsManifestDownload: 'Downloading assets manifest',
  ClientCheck: 'Checking client jar',
  ContentResolve: 'Resolving content',
  Done: 'Starting',
  JavaManifestCheck: 'Checking java manifest',
  JavaManifestDownload: 'Downloading java manifest',
  ModLoaderFilesDownloadInfo: 'Downloading mod loader file information',
  ModLoaderMeta: 'Downloading mod loader version meta',
  ModLoaderPreprocess: 'Running mod loader preprocessing',
  ModLoaderPreprocessDone: 'Mod loader preprocessing done',
  VersionManifestCheck: 'Checking version manifest',
  VersionManifestDownload: 'Downloading version manifest'
};

const PROGRESS_STAGES: Record<string, string> = {
  AssetsCheck: 'Checking assets',
  AssetsDownload: 'Downloading assets',
  ClientDownload: 'Downloading client',
  ContentDownload: 'Downloading content',
  ContentFiles: 'Installing content',
  JavaCheck: 'Checking java files',
  JavaDownload: 'Downloading java files',
  LibraryCheck: 'Checking libraries',
  LibraryDownload: 'Downloading libraries',
  ModLoaderFilesCheck: 'Checking mod loader files',
  ModLoaderFilesDownload: 'Downloading mod loader files',
  ModpackOverrides: 'Applying modpack overrides',
  NativeLibraryCheck: 'Checking native libraries',
  NativeLibraryDownload: 'Downloading native libraries'
};

const UNITS: Record<string, string> = {
  ContentFiles: 'files'
};

export const parse_stage = (status: VersionCheckStatus): Stage | undefined => {
  if (typeof status === 'string') {
    const stage = SIMPLE_STAGES[status];
    return stage === undefined ? undefined : { stage };
  }
  const [key, progress] = Object.entries(status)[0] ?? [];
  const stage = key === undefined ? undefined : PROGRESS_STAGES[key];
  if (stage === undefined) {
    return undefined;
  }
  return {
    bytes: key.endsWith('Download'),
    progress,
    stage,
    ...(UNITS[key] ? { unit: UNITS[key] } : {})
  };
};

export const new_download = (
  id: number,
  title: string,
  now: number
): Download => ({
  bytes: false,
  id,
  stage: 'Checking version manifest',
  started_at: now,
  status: 'running',
  title
});

/** Weight of the newest sample in the speed moving average */
const SMOOTHING = 0.3;

export const apply_status = (
  download: Download,
  status: VersionCheckStatus,
  now: number
): Download => {
  const parsed = parse_stage(status);
  if (download.status !== 'running' || !parsed) {
    return download;
  }
  const [value, total] = parsed.progress ?? [];
  const bytes = parsed.bytes ?? false;
  const next: Download = {
    ...download,
    bytes,
    speed: undefined,
    stage: parsed.stage,
    total,
    unit: parsed.unit,
    updated_at: bytes ? now : undefined,
    value
  };
  if (status === 'Done') {
    return { ...next, total: 1, value: 1 };
  }
  const same_stage = bytes && download.bytes && download.stage === parsed.stage;
  if (!same_stage || value === undefined) {
    return next;
  }
  const dt = (now - (download.updated_at ?? now)) / 1000;
  const dv = value - (download.value ?? 0);
  if (dt <= 0 || dv < 0) {
    return { ...next, speed: download.speed, updated_at: download.updated_at };
  }
  const sample = dv / dt;
  return {
    ...next,
    speed:
      download.speed === undefined
        ? sample
        : download.speed * (1 - SMOOTHING) + sample * SMOOTHING
  };
};

export const percent = (download: Download) => {
  if (download.status === 'done') {
    return 100;
  }
  if (download.value === undefined || !download.total) {
    return 0;
  }
  return Math.min(100, Math.round((download.value / download.total) * 100));
};

export const percent_label = (download: Download) => {
  switch (download.status) {
    case 'done': {
      return 'Done';
    }
    case 'error': {
      return 'Failed';
    }
    case 'canceled': {
      return 'Canceled';
    }
    default: {
      return download.value === undefined ? '' : `${percent(download)}%`;
    }
  }
};

const mib = (bytes: number) => {
  const value = bytes / 1024 / 1024;
  return value < 10 ? value.toFixed(1) : String(Math.round(value));
};

export const detail = (download: Download) => {
  if (download.status !== 'running') {
    return '';
  }
  const { value, total, stage } = download;
  if (value === undefined || total === undefined) {
    return stage;
  }
  const amount = download.bytes
    ? `${mib(value)} of ${mib(total)} MiB`
    : `${value.toLocaleString('en-US')} of ${total.toLocaleString('en-US')}${
        download.unit ? ` ${download.unit}` : ''
      }`;
  return `${stage} · ${amount}`;
};

export const speed_label = (download: Download) =>
  download.status === 'running' && download.bytes && download.speed
    ? `${(download.speed / 1_000_000).toFixed(1)} MB/s`
    : '';

export const clear_finished = (downloads: Download[]) =>
  downloads.filter((d) => d.status === 'running');

export const running_count = (downloads: Download[]) =>
  downloads.filter((d) => d.status === 'running').length;

export const chip_label = (running: number) => {
  if (running === 0) {
    return 'Downloads';
  }
  return running === 1 ? '1 download' : `${running} downloads`;
};

/** Average percent of the running downloads, for the chip bar */
export const running_percent = (downloads: Download[]) => {
  const running = downloads.filter((d) => d.status === 'running');
  return running.length === 0
    ? 100
    : running.reduce((sum, d) => sum + percent(d), 0) / running.length;
};

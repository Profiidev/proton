import { describe, expect, it } from 'vitest';
import {
  type Download,
  apply_status,
  chip_label,
  clear_finished,
  detail,
  new_download,
  parse_stage,
  percent,
  percent_label,
  running_count,
  running_percent,
  speed_label
} from '$lib/downloads';

const MiB = 1024 * 1024;
const base = () => new_download(1, 'Survival: launch', 0);

describe('parse_stage', () => {
  it('maps simple stages', () => {
    expect(parse_stage('ClientCheck')).toEqual({
      stage: 'Checking client jar'
    });
  });
  it('maps progress stages and flags byte stages', () => {
    expect(parse_stage({ AssetsDownload: [1, 2] })).toEqual({
      bytes: true,
      progress: [1, 2],
      stage: 'Downloading assets'
    });
    expect(parse_stage({ LibraryCheck: [3, 4] })?.bytes).toBe(false);
  });
  it('ignores unknown stages', () => {
    // oxlint-disable-next-line no-unsafe-type-assertion
    expect(parse_stage('Nope' as 'ClientCheck')).toBeUndefined();
  });
});

describe('apply_status', () => {
  it('updates stage and counts', () => {
    const d = apply_status(base(), { LibraryCheck: [120, 300] }, 10);
    expect(d).toMatchObject({
      bytes: false,
      stage: 'Checking libraries',
      total: 300,
      value: 120
    });
    expect(d.speed).toBeUndefined();
  });

  it('computes speed from deltas within a byte stage', () => {
    let d = apply_status(base(), { AssetsDownload: [0, 100 * MiB] }, 1000);
    expect(d.speed).toBeUndefined();
    d = apply_status(d, { AssetsDownload: [2_000_000, 100 * MiB] }, 2000);
    expect(d.speed).toBe(2_000_000);
    d = apply_status(d, { AssetsDownload: [6_000_000, 100 * MiB] }, 3000);
    // Smoothed: 0.7 * 2e6 + 0.3 * 4e6
    expect(d.speed).toBeCloseTo(2_600_000);
  });

  it('resets speed when the stage changes', () => {
    let d = apply_status(base(), { AssetsDownload: [0, 10] }, 0);
    d = apply_status(d, { AssetsDownload: [5, 10] }, 1000);
    expect(d.speed).toBe(5);
    d = apply_status(d, { LibraryDownload: [1, 10] }, 2000);
    expect(d.speed).toBeUndefined();
  });

  it('keeps speed on zero time delta', () => {
    let d = apply_status(base(), { ClientDownload: [0, 10] }, 0);
    d = apply_status(d, { ClientDownload: [5, 10] }, 1000);
    d = apply_status(d, { ClientDownload: [6, 10] }, 1000);
    expect(d.speed).toBe(5);
    expect(d.value).toBe(6);
  });

  it('marks Done as full progress', () => {
    const d = apply_status(base(), 'Done', 0);
    expect(percent(d)).toBe(100);
    expect(d.status).toBe('running');
  });

  it('ignores events for finished downloads', () => {
    const done: Download = { ...base(), status: 'done' };
    expect(apply_status(done, 'ClientCheck', 0)).toBe(done);
  });
});

describe('labels', () => {
  it('percent', () => {
    expect(percent(base())).toBe(0);
    expect(percent({ ...base(), total: 3, value: 1 })).toBe(33);
    expect(percent({ ...base(), total: 0, value: 0 })).toBe(0);
    expect(percent({ ...base(), status: 'done' })).toBe(100);
  });

  it('percent_label', () => {
    expect(percent_label(base())).toBe('');
    expect(percent_label({ ...base(), total: 4, value: 1 })).toBe('25%');
    expect(percent_label({ ...base(), status: 'done' })).toBe('Done');
    expect(percent_label({ ...base(), status: 'error' })).toBe('Failed');
    expect(percent_label({ ...base(), status: 'canceled' })).toBe('Canceled');
  });

  it('detail', () => {
    const at = (status: Parameters<typeof apply_status>[1]) =>
      detail(apply_status(base(), status, 0));
    expect(detail(base())).toBe('Checking version manifest');
    expect(at({ AssetsDownload: [41 * MiB, 96 * MiB] })).toBe(
      'Downloading assets · 41 of 96 MiB'
    );
    expect(at({ ClientDownload: [MiB / 2, 2 * MiB] })).toBe(
      'Downloading client · 0.5 of 2.0 MiB'
    );
    expect(at({ AssetsCheck: [2904, 3580] })).toBe(
      'Checking assets · 2,904 of 3,580'
    );
    expect(detail({ ...base(), status: 'done' })).toBe('');
  });

  it('speed_label only for running byte stages', () => {
    const d = { ...base(), bytes: true, speed: 3_200_000 };
    expect(speed_label(d)).toBe('3.2 MB/s');
    expect(speed_label({ ...d, bytes: false })).toBe('');
    expect(speed_label({ ...d, status: 'done' })).toBe('');
  });

  it('chip_label', () => {
    expect(chip_label(0)).toBe('Downloads');
    expect(chip_label(1)).toBe('1 download');
    expect(chip_label(2)).toBe('2 downloads');
  });
});

describe('list helpers', () => {
  const list: Download[] = [
    { ...base(), id: 1, total: 4, value: 1 },
    { ...base(), id: 2, status: 'done' },
    { ...base(), id: 3, status: 'error' },
    { ...base(), id: 4, status: 'canceled' },
    { ...base(), id: 5, total: 4, value: 3 }
  ];

  it('clear_finished keeps running only', () => {
    expect(clear_finished(list).map((d) => d.id)).toEqual([1, 5]);
  });

  it('running_count', () => {
    expect(running_count(list)).toBe(2);
    expect(running_count([])).toBe(0);
  });

  it('running_percent averages running downloads', () => {
    expect(running_percent(list)).toBe(50);
    expect(running_percent([])).toBe(100);
  });
});

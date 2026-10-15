import { describe, expect, it } from 'vitest';
import {
  instance_subtitle,
  memory_usage,
  running_summary
} from '$lib/components/instance/instance';

describe('instance row helpers', () => {
  it('pluralizes the running summary', () => {
    expect(running_summary(1, 1)).toBe('1 game running across 1 profile.');
    expect(running_summary(3, 2)).toBe('3 games running across 2 profiles.');
  });

  it('computes memory label and percent', () => {
    const gb = 1024 ** 3;
    expect(memory_usage(undefined, 8192)).toBeUndefined();
    expect(memory_usage(2 * gb, 8192)).toEqual({
      label: '2.0 of 8.0 GB',
      percent: 25
    });
    expect(memory_usage(16 * gb, 8192)?.percent).toBe(100);
    expect(memory_usage(gb)).toEqual({ label: '1.0 GB', percent: 0 });
  });

  it('builds the subtitle with optional account', () => {
    expect(instance_subtitle('5m', 'Steve', true)).toBe(
      'Playing as Steve · running for 5m'
    );
    expect(instance_subtitle('5m', 'Steve')).toBe('Steve · running for 5m');
    expect(instance_subtitle('5m', undefined, true)).toBe('running for 5m');
  });
});

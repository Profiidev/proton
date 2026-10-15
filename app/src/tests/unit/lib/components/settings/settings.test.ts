import { describe, expect, it } from 'vitest';
import {
  add_env_var,
  memory_of_total,
  memory_with_mb,
  parse_args,
  remove_env_var,
  sorted_env
} from '$lib/components/settings/settings';

describe('memory labels', () => {
  it('formats memory of total', () => {
    expect(memory_of_total(6144, 14_016)).toBe('6.0 GB of 13.7 GB');
  });

  it('formats memory with MB', () => {
    expect(memory_with_mb(14_016)).toBe('13.7 GB (14016 MB)');
  });
});

describe('parse_args', () => {
  it('splits on whitespace and drops empties', () => {
    expect(parse_args('  -XX:+UseG1GC   -Xss1M ')).toEqual([
      '-XX:+UseG1GC',
      '-Xss1M'
    ]);
    expect(parse_args('   ')).toEqual([]);
  });
});

describe('env vars', () => {
  it('adds a trimmed entry without mutating', () => {
    const env = { A: '1' };
    expect(add_env_var(env, ' B ', ' 2 ')).toEqual({ A: '1', B: '2' });
    expect(env).toEqual({ A: '1' });
  });

  it('rejects empty, duplicate and invalid keys', () => {
    expect(typeof add_env_var({}, '', 'x')).toBe('string');
    expect(typeof add_env_var({}, 'A', ' ')).toBe('string');
    expect(typeof add_env_var({ A: '1' }, 'A', '2')).toBe('string');
    expect(typeof add_env_var({}, 'A=B', '2')).toBe('string');
  });

  it('removes an entry without mutating', () => {
    const env = { A: '1', B: '2' };
    expect(remove_env_var(env, 'A')).toEqual({ B: '2' });
    expect(env).toEqual({ A: '1', B: '2' });
  });

  it('sorts by key', () => {
    expect(
      sorted_env(
        Object.fromEntries([
          ['b', '2'],
          ['a', '1']
        ])
      )
    ).toEqual([
      ['a', '1'],
      ['b', '2']
    ]);
  });
});

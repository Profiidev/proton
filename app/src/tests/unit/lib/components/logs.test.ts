import { describe, expect, it } from 'vitest';
import {
  CONSOLE_LEVELS,
  filter_logs,
  parse_line,
  parse_logs
} from '$lib/components/logs';

describe('parse_line', () => {
  it('parses a line without sub location', () => {
    expect(
      parse_line('[17:48:10] [Render thread/INFO]: Setting user: ProfiDev')
    ).toEqual({
      level: 'INFO',
      location: 'Render thread',
      message: 'Setting user: ProfiDev',
      subLocation: undefined,
      time: '17:48:10'
    });
  });

  it('parses a line with sub location', () => {
    expect(
      parse_line('[17:48:02] [main/WARN] [minecraft/Main]: Slow start')
    ).toEqual({
      level: 'WARN',
      location: 'main',
      message: 'Slow start',
      subLocation: 'minecraft/Main',
      time: '17:48:02'
    });
  });

  it('keeps an unparsable line as message', () => {
    expect(parse_line('\tat java.lang.Thread.run')).toEqual({
      level: '',
      location: '',
      message: '\tat java.lang.Thread.run',
      time: ''
    });
  });
});

describe('filter_logs', () => {
  const entries = [
    '[1] [Server thread/INFO]: Saving the game',
    "[2] [Server thread/WARN]: Can't keep up!",
    '[3] [Render thread/ERROR]: Failed to load sound',
    '[4] [Render thread/DEBUG] [Iris]: Loaded shader pack',
    'stack trace line'
  ].map(parse_line);
  const times = (levels: string[], search = '') =>
    filter_logs(entries, search, levels).map((e) => e.time || e.message);

  it('keeps everything without filters', () => {
    expect(times([])).toHaveLength(5);
  });

  it('applies the console modes', () => {
    expect(times(CONSOLE_LEVELS.all)).toHaveLength(5);
    expect(times(CONSOLE_LEVELS.warn)).toEqual(['2', '3']);
    expect(times(CONSOLE_LEVELS.err)).toEqual(['3']);
  });

  it('filters by multiple chosen levels', () => {
    expect(times(['INFO', 'DEBUG'])).toEqual(['1', '4']);
  });

  it('searches message, location and sub location case insensitively', () => {
    expect(times([], 'SAVING')).toEqual(['1']);
    expect(times([], 'render')).toEqual(['3', '4']);
    expect(times([], 'iris')).toEqual(['4']);
    expect(times([], 'trace')).toEqual(['stack trace line']);
  });

  it('combines search and level filter', () => {
    expect(times(['ERROR'], 'render')).toEqual(['3']);
  });
});

describe('parse_logs', () => {
  const entries = parse_logs([
    'orphan before any level',
    '[1] [Render thread/ERROR]: Crash',
    'java.lang.RuntimeException: boom',
    '\tat Foo.bar',
    '[2] [Render thread/INFO]: Recovered'
  ]);

  it('marks unparsable lines as continuation with the previous level', () => {
    expect(entries.map((e) => [e.level, Boolean(e.continuation)])).toEqual([
      ['', true],
      ['ERROR', false],
      ['ERROR', true],
      ['ERROR', true],
      ['INFO', false]
    ]);
    expect(entries[2].time).toBe('');
  });

  it('keeps the stack trace in the errors view', () => {
    expect(
      filter_logs(entries, '', CONSOLE_LEVELS.err).map((e) => e.message)
    ).toEqual(['Crash', 'java.lang.RuntimeException: boom', '\tat Foo.bar']);
  });
});

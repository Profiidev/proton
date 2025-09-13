import { DateTime } from 'positron-components/util';
import type { Profile } from './tauri/profile.svelte';

export const file_to_bytes = (file: File) => {
  return new Promise<Uint8Array>((resolve) => {
    let reader = new FileReader();
    reader.onload = (e) => {
      const arrayBuffer = e.target?.result;
      if (!(arrayBuffer instanceof ArrayBuffer)) return;
      resolve(new Uint8Array(arrayBuffer));
    };

    reader.readAsArrayBuffer(file);
  });
};

export const rem_to_px = (rem: number) => {
  const rootFontSize = parseFloat(
    getComputedStyle(document.documentElement).fontSize
  );
  return rem * rootFontSize;
};

export const b_to_mb = (bytes: number) => {
  return bytes / 1024 / 1024;
};

export const debounce = <T extends (...args: any[]) => void>(
  func: T,
  delay: number
) => {
  let timeout: number | undefined = undefined;

  const debounced = function (
    this: ThisParameterType<T>,
    ...args: Parameters<T>
  ) {
    const context = this;

    const later = () => {
      timeout = undefined;
      func.apply(context, args);
    };

    if (timeout) clearTimeout(timeout);
    timeout = setTimeout(later, delay);
  };

  return debounced as T;
};

export const compareDateTimes = (a: string, b: string) => {
  return DateTime.fromISO(a).diff(DateTime.fromISO(b)).milliseconds > 0
    ? -1
    : 1;
};

export const compareProfiles = (a: Profile, b: Profile) => {
  if (!a.last_played && !b.last_played) {
    return compareDateTimes(a.created_at, b.created_at);
  }
  if (a.last_played && b.last_played) {
    return compareDateTimes(a.last_played, b.last_played);
  }
  if (a.last_played) {
    return -1;
  }
  return 1;
};

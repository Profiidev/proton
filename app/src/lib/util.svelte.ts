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

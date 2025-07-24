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

export const levenshtein = (a: string, b: string): number => {
  const an = a.length;
  const bn = b.length;

  if (an === 0) return bn;
  if (bn === 0) return an;

  const matrix = new Array<number[]>(bn + 1);
  for (let i = 0; i <= bn; i++) {
    matrix[i] = new Array<number>(an + 1);
    matrix[i][0] = i;
  }

  const firstRow = matrix[0];
  for (let j = 1; j <= an; j++) {
    firstRow[j] = j;
  }

  for (let i = 1; i <= bn; i++) {
    for (let j = 1; j <= an; j++) {
      if (b.charAt(i - 1) === a.charAt(j - 1)) {
        matrix[i][j] = matrix[i - 1][j - 1];
      } else {
        matrix[i][j] =
          Math.min(
            matrix[i - 1][j - 1], // Substitution
            matrix[i][j - 1], // Insertion
            matrix[i - 1][j] // Deletion
          ) + 1;
      }
    }
  }

  return matrix[bn][an];
};

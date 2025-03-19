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

// Reads the pixels of a chart snapshot (an ImageBitmap) off the main thread
// for the viewer's dust transition (lib/engine.js `snapshot`).
self.onmessage = ({ data: { id, bitmap, width, height } }) => {
  try {
    const canvas = new OffscreenCanvas(width, height);
    const g = canvas.getContext('2d', { willReadFrequently: true });
    g.drawImage(bitmap, 0, 0, width, height);
    bitmap.close();
    const { data } = g.getImageData(0, 0, width, height);
    self.postMessage({ id, pixels: data.buffer }, [data.buffer]);
  } catch {
    self.postMessage({ id, pixels: null });
  }
};

import { LocalDataset, LocalDicomInstanceDecoder } from '../dist/index';

// Manage drag and drop of DICOM files onto the canvas.
function setupCanvas(id: string,
  useBufferFn: (canvas: HTMLCanvasElement, buffer: ArrayBuffer) => void): void {
  const canvas = document.getElementById(id) as HTMLCanvasElement;
  canvas.width = 512;
  canvas.height = 512;
  canvas.addEventListener("dragover", event => {
    // prevent default to allow drop
    event.preventDefault();
  });
  canvas.addEventListener('drop', async event => {
    // prevent default to allow drop
    event.preventDefault();

    if (event.dataTransfer === null) {
      throw new Error('Something went wrong when the file was dropped');
    }
    const file = event.dataTransfer.files[0];
    const fileReader = new FileReader();
    const buffer = await new Promise<ArrayBuffer>(resolve => {
      fileReader.onload = () => resolve(fileReader.result as ArrayBuffer);
      fileReader.onerror = e => console.error(e);
      fileReader.readAsArrayBuffer(file);
    });
    useBufferFn(canvas, buffer);
  });
}

function ensureDefined<T>(value: T | undefined, name: string): T {
  if (value === undefined || value === null) {
    throw new Error(`{$name} is undefined`);
  }
  return value;
}

async function main(): Promise<void> {
  console.log('ready');
  const instanceDecoder = new LocalDicomInstanceDecoder();
  console.log(`${instanceDecoder.memory.buffer.byteLength / 1024} KB allocated (${instanceDecoder.memory.buffer.byteLength})`);
  await instanceDecoder.init('rdicom.debug.wasm');

  setupCanvas('vp1', async (canvas, buffer) => {
    const instance = instanceDecoder.getInstanceFromBuffer(buffer);
    const localDataset = new LocalDataset(instanceDecoder, instance);
    (window as any).localDataset = localDataset;
    const columns = localDataset.getColumns();
    const rows = localDataset.getRows();
    console.log(`${columns}x${rows}`);
    const pixels = await localDataset.getPixelData();

    const imageCanvas = document.createElement('canvas');
    imageCanvas.width = columns;
    imageCanvas.height = rows;
    const imageCtx = ensureDefined(imageCanvas.getContext('2d'), 'imageCtx') as CanvasRenderingContext2D;
    const imageData = imageCtx.getImageData(0, 0, imageCanvas.width, imageCanvas.height);
    for (let i = 0; i < imageData.data.length / 4; ++i) {
      imageData.data[i * 4    ] = pixels[i * 3    ];
      imageData.data[i * 4 + 1] = pixels[i * 3 + 1];
      imageData.data[i * 4 + 2] = pixels[i * 3 + 2];
      imageData.data[i * 4 + 3] = 0xFF;
    }
    imageCtx.putImageData(imageData, 0, 0);
    const ctx = ensureDefined(canvas.getContext('2d'), 'imageCtx') as CanvasRenderingContext2D;
    if (columns / rows > canvas.width / canvas.height) {
      const zoom = canvas.width / columns;
      ctx.setTransform(
        zoom, 0, 0, zoom, 0,
        Math.round(canvas.height / 2) - Math.round(zoom * rows / 2),
      );
    } else {
      const zoom = canvas.height / rows;
      ctx.setTransform(
        zoom, 0, 0, zoom,
        Math.round(canvas.width / 2) - Math.round(zoom * columns / 2), 0,
      );
    }
    ctx.drawImage(imageCanvas, 0, 0);
  });
}

window.onload = main;

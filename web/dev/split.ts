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
  await instanceDecoder.init(/* 'rdicom.debug.wasm' */);

  setupCanvas('vp1', async (canvas, buffer) => {
    console.log(buffer);
    // const instance = instanceDecoder.getInstanceFromBuffer(buffer);
    // const localDataset = new LocalDataset(instanceDecoder, instance);
    // (window as any).localDataset = localDataset;
    // const columns = localDataset.getColumns();
    // const rows = localDataset.getRows();
    // const pixels = await localDataset.getPixelData();

    // const imageCanvas = document.createElement('canvas');
    // imageCanvas.width = columns;
    // imageCanvas.height = rows;
  });
}

window.onload = main;

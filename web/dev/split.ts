import { LocalDataset, LocalDicomInstanceDecoder } from '../dist/index';

// Manage drag and drop of DICOM files onto the canvas.
function setupCanvas(id: string,
  useBufferFn: (canvas: HTMLCanvasElement, nbfiles: number, buffer: ArrayBuffer) => void): void {
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

    const nbfiles = event.dataTransfer?.files.length ?? 0;
    Array.from(event.dataTransfer.files).map(async file => {
      const fileReader = new FileReader();
      const buffer = await new Promise<ArrayBuffer>(resolve => {
        fileReader.onload = () => resolve(fileReader.result as ArrayBuffer);
        fileReader.onerror = e => console.error(e);
        fileReader.readAsArrayBuffer(file);
      });
      useBufferFn(canvas, nbfiles, buffer);
    });
  });
}

function ensureDefined<T>(value: T | undefined, name: string): T {
  if (value === undefined || value === null) {
    throw new Error(`{$name} is undefined`);
  }
  return value;
}

type InstanceSet = { [key: string]: LocalDataset[] };

function splitSeries(instances: InstanceSet): LocalDataset[] {
  const instancesToSplit = instances[Object.keys(instances)[0]];
  // Compute the direction of the volume based on the first frame
  const firstInstance = instancesToSplit[0];
  const imageOrientationPatient = firstInstance.getImageOrientationPatient();
  console.log(imageOrientationPatient);
  instancesToSplit.reduce((acc, value) => {
    return acc;
  }, );
  return [];
}

async function main(): Promise<void> {
  console.log('ready');
  const instanceDecoder = new LocalDicomInstanceDecoder(700 * 1024 / 64);
  console.log(`${instanceDecoder.memory.buffer.byteLength / 1024} KB allocated (${instanceDecoder.memory.buffer.byteLength})`);
  await instanceDecoder.init('rdicom.debug.wasm');

  const statusLine = document.getElementById('status') as HTMLDivElement;
  let count = 0;
  const instances: InstanceSet = {};
  setupCanvas('vp1', async (canvas, nbfiles, buffer) => {
    count += 1;
    const instance = instanceDecoder.getInstanceFromBuffer(buffer);
    const localDataset = new LocalDataset(instanceDecoder, instance);
    instances[localDataset.getSeriesInstanceUID()] ??= [];
    instances[localDataset.getSeriesInstanceUID()].push(localDataset);
    (window as any).localDataset = localDataset;
    statusLine.innerText = `${count} / ${nbfiles} (${Math.round(count / nbfiles * 100)}%)`;

    if (count === nbfiles) {
      splitSeries(instances);
    }
    // const imageCanvas = document.createElement('canvas');
    // imageCanvas.width = columns;
    // imageCanvas.height = rows;
  });

  (window as any).instances = instances;
}

window.onload = main;

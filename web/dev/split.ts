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

    console.time("Loading time");

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

type Series = LocalDataset[];
type SeriesSet = { [key: string]: LocalDataset[] };

interface Vector {
  normalize(): Vector;
  dot(v: number[]): number;
}
declare const math: { cross(u: number[], v: number[]): Vector };

function splitSeries(series: SeriesSet): Series[][] {
  return Object.keys(series).map(seriesUID => {
    const instances = series[seriesUID];
    // Compute the direction of the volume based on the first frame
    const firstInstance = instances[0];
    const imageOrientationPatient = firstInstance.getImageOrientationPatient();
    const [x1, x2, x3, y1, y2, y3] = imageOrientationPatient as number[];
    const direction = math.cross([x1, x2, x3], [y1, y2, y3]).normalize();
    const sortedInstances = instances.reduce((acc: { position: number, instance: LocalDataset }[], instance) => {
      const imagePositionPatient = instance.getImagePositionPatient();
      const position = direction.dot(imagePositionPatient);
      acc.push({ position, instance });
      return acc;
    }, []).sort((a, b) => a.position > b.position ? 1 : -1).map(entry => entry.instance);
    // Split the instances
    const stacks = [[sortedInstances[0]]];
    sortedInstances.slice(1).map(frame => {
      const instanceNumber = frame.getInstanceNumber();
      const stack = stacks.find(g => {
        const stackInstanceNumber = g[g.length - 1].getInstanceNumber();
        // The last instance of the stack and the instance are next to each other
        return Math.abs(instanceNumber - stackInstanceNumber) === 1;
      });
      if (stack !== undefined) {
        stack.push(frame);
      } else {
        stacks.push([frame]);
      }
    });
    return stacks;
  });
}

interface LUTData {
  paddingValue: number,
  slope: number,
  intercept: number,
}

async function main(): Promise<void> {
  console.log('ready');
  const instanceDecoder = await LocalDicomInstanceDecoder.create({
    nbpages: 700 * 1024 / 64,
    // rdicompath: rdicom.debug.wasm,
  });
  console.log(`${instanceDecoder.memory.buffer.byteLength / 1024} KB allocated (${instanceDecoder.memory.buffer.byteLength})`);

  const statusLine = document.getElementById('status') as HTMLDivElement;
  let count = 0;
  const instances: SeriesSet = {};
  setupCanvas('vp1', async (canvas, nbfiles, buffer) => {
    count += 1;
    const instance = instanceDecoder.getInstanceFromBuffer(buffer);
    const localDataset = new LocalDataset(instanceDecoder, instance);
    instances[localDataset.getSeriesInstanceUID()] ??= [];
    instances[localDataset.getSeriesInstanceUID()].push(localDataset);
    (window as any).localDataset = localDataset;
    statusLine.innerText = `${count} / ${nbfiles} (${Math.round(count / nbfiles * 100)}%)`;

    if (count === nbfiles) {
      console.timeEnd("Loading time");

      console.time("Split time");
      const series = splitSeries(instances);
      console.timeEnd("Split time");
      (window as any).series = series;

      // Display first stack of the first series
      const stack = series[0][0];
      const columns = stack[0].getColumns();
      const rows = stack[0].getRows();
      const imageCanvas = document.createElement('canvas');
      imageCanvas.width = columns;
      imageCanvas.height = rows;

      const imageCtx = ensureDefined(imageCanvas.getContext('2d'), 'imageCtx') as CanvasRenderingContext2D;
      const imageData = imageCtx.getImageData(0, 0, imageCanvas.width, imageCanvas.height);

      // Compute VOI parameters
      const paddingValueUS = stack[0].getPixelPaddingValue();
      // TODO: Need to make this conversion in the local-dataset.ts
      const paddingValue = stack[0].getPixelRepresentation() === 1
        ? -(Math.pow(2, stack[0].getBitsStored()) - paddingValueUS)
        : paddingValueUS
      const modalitySlope = stack[0].getRescaleSlope();
      const modalityIntercept = stack[0].getRescaleIntercept();
      const wc = stack[0].getWindowCenter()[0];
      const ww = stack[0].getWindowWidth()[0];
      const voiSlope = 255 / ww;
      const voiIntercept = 128 - (wc * voiSlope);

      const lutData = {
        paddingValue,
        slope: modalitySlope * voiSlope,
        intercept: voiSlope * modalityIntercept + voiIntercept,
      };
      let index = 0;
      await showImage(canvas, imageCtx, imageData, stack[index], lutData);
      canvas.addEventListener("wheel", (event) => {
        index = Math.min(stack.length - 1, Math.max(0, event.deltaY > 0 ? index + 1 : index - 1));
        showImage(canvas, imageCtx, imageData, stack[index], lutData);
      });

      async function showImage(canvas: HTMLCanvasElement, imageCtx: CanvasRenderingContext2D,
        imageData: ImageData, instance: LocalDataset, lutData: LUTData): Promise<void> {
        // We will assume signed 16 bits per pixels
        const pixelData = await instance.getPixelData();
        const pixels = new Int16Array(pixelData.buffer, pixelData.byteOffset, columns * rows);
        // const pixels = new Int16Array(instanceDecoder.memory.buffer, pixelData.byteOffset, columns * rows);
        // Display series 0
        for (let i = 0; i < imageData.data.length / 4; ++i) {
          const value = pixels[i] !== lutData.paddingValue
            ? Math.max(0, Math.min(255, (pixels[i] * lutData.slope + lutData.intercept) | 0))
            : 0;
          imageData.data[i * 4    ] = value;
          imageData.data[i * 4 + 1] = value;
          imageData.data[i * 4 + 2] = value;
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
      }
    }
  });

  (window as any).instances = instances;
}

window.onload = main;

// https://surma.dev/things/rust-to-webassembly/

const textDecoder = new TextDecoder();

// From a position in a buffer, assume a null terminated c-string and return
// a javascript string.
function toStr(charArray, ptr, limit=255) {
  let end = ptr;
  while (charArray[end++] && (end - ptr) < limit);
  return textDecoder.decode(new Uint8Array(charArray.buffer, ptr, end - ptr - 1));
}

let gmemory;
async function rdicomInit(rdicom_path) {
  // By default, memory is 1 page (64K). We'll need a little more
  const memory = new WebAssembly.Memory({ initial: 1000 });
  console.log(`${memory.buffer.byteLength / 1024} KB allocated (${memory.buffer.byteLength})`);
  gmemory = memory;

  // Position in memory of the next available free byte.
  // malloc will move that position.
  let heapPos = 1; // 0 is the NULL pointer. Not a proper malloc return value...
  // log string buffer
  let str = '';
  // These are the functions for the WASM environment available for the zig code
  // to communicate with the JS environment.
  const env = {
    memory,
    log: console.log,
    // Add a log string to the buffer
    addString: (offset, size) => {
      console.log('offset', offset.toString(16), 'size', size);
      window.view = new Uint8Array(memory.buffer);
      // for (let i = 0; i < view.length; ++i) {
      //   console.log(i, view[i]);
      // }
      str = str + textDecoder.decode(new Uint8Array(memory.buffer, offset, size));
    },
    // Flush the log string buffer with console.log
    printString: () => {
      console.log('rdicom:', str);
      str = '';
    },
    // Flush the log string buffer with console.log
    printError: () => {
      console.error('rdicom:', str);
      str = '';
    },
     // libc memset reimplementation
    memset: (ptr, value, size) => {
      // console.log('memset');
      const mem = new Uint8Array(memory.buffer);
      mem.fill(value, ptr, ptr + size);
      return ptr;
    },
    // libc memcpy reimplementation
    memcpy: (dest, source, n) => {
      // console.log('memcpy');
      const mem = new Uint8Array(memory.buffer);
      mem.copyWithin(dest, source, source + n);
      return dest;
    },
    // libc memcmp reimplmentation
    memcmp: (s1, s2, n) => {
      // console.log('memcmp');
      const charArray = new Uint8Array(memory.buffer);
      for (let i = 0; i < n; i++) {
        if (charArray[s1] !== charArray[s2]) {
          return charArray[s1] - charArray[s2];
        }
      }
      return 0;
    },
    // libc malloc reimplementation
    // This dumb allocator just churn through the memory and does not keep
    // track of freed memory. Will work for a while...
    malloc: (size, align = 1) => {
      let ptr = heapPos;
      heapPos += size;
      // Align ptr
      let mod = ptr % align;
      if (mod !== 0) {
        const move_to_align = align - (ptr % align);
        ptr += move_to_align;
        heapPos += move_to_align;
      }
      // console.log(`malloc(${size}, ${align})`, `-> 0x${ptr.toString(16)} (${ptr})`);
      return ptr;
    },
    // libc free reimplementation
    free: ptr => {
      // console.log(`free 0x${ptr.toString(16)} (${ptr})`);
      // Nothing gets freed
    },
    __assert_fail_js: (assertion, file, line, fun) => {
      const charArray = new Uint8Array(memory.buffer);
      console.log(`${toStr(charArray, file)}(${line}): ${toStr(charArray, assertion)} in ${toStr(charArray, fun)}`);
    },
  }
  // Load the wasm code
  const rdicom = await WebAssembly.instantiateStreaming(fetch(rdicom_path), { env });
  rdicom.env = env;
  window.rdicom = rdicom;
  heapPos = rdicom.instance.exports.__heap_base.value;
  console.log(`__heap_base.value ${heapPos}`);

  console.log('rdicom loaded')
  return rdicom;
}

const fileReader = new FileReader();

function setupCanvas(id, useBufferFn) {
  const canvas = document.getElementById(id);
  canvas.width = 512;
  canvas.height = 512;
  canvas.addEventListener("dragover", event => {
    // prevent default to allow drop
    event.preventDefault();
  });
  canvas.addEventListener('drop', async event => {
    // prevent default to allow drop
    event.preventDefault();

    const file = event.dataTransfer.files[0];
    console.log('file', file);
    const buffer = await new Promise(resolve => {
      fileReader.onload = () => resolve(fileReader.result);
      fileReader.onerror = e => console.error(e);
      fileReader.readAsArrayBuffer(file);
    });
    console.log('buffer', buffer);
    useBufferFn(canvas, buffer);
  });
}

function getInstanceFromBuffer(rdicom, buffer) {
  const { instance_from_ptr } = rdicom.instance.exports;
  // Allocate memory and ptr points to index at which the allocated buffer starts
  const ptr = rdicom.env.malloc(buffer.byteLength);
  console.log('ptr', ptr);
  // Map the whole wasm memory
  const memory = new Uint8Array(rdicom.env.memory.buffer);
  // Set the content of the DICOM file to the wasm memory at index ptr
  memory.set(new Uint8Array(buffer), ptr);
  const handle = instance_from_ptr(ptr, buffer.byteLength);
  console.log(`handle ${handle}`);
  return handle;
}

function getValue(rdicom, instance, tag) {
  const { get_value_from_ptr } = rdicom.instance.exports;
  return get_value_from_ptr(instance, tag);
}

function fromCString(rdicom, offset) {
  const memory = new Uint8Array(rdicom.env.memory.buffer);
  let zero = offset;
  while (memory[zero] !== 0) zero++;
  return textDecoder.decode(new Uint8Array(memory.buffer, offset, zero - offset));
}

function fromF64(rdicom, offset) {
  const memory = new Float64Array(rdicom.env.memory.buffer, offset)
  return memory[0];
}

function fromArrayBuffer(rdicom, offset) {
  const vector = new Uint32Array(rdicom.env.memory.buffer, offset);
  console.log(vector[0], vector[1]);
  return new Uint8Array(rdicom.env.memory.buffer, vector[1], vector[0]);
}

async function main() {
  const rdicom = await rdicomInit('rdicom.wasm');
  let instance; // the DICOM instance
  setupCanvas('vp1', (canvas, buffer) => {
    instance = getInstanceFromBuffer(rdicom, buffer);
    console.log('instance loaded');
    let value = getValue(rdicom, instance, 0x0020000d);
    const StudyInstanceUID = fromCString(rdicom, value);
    console.log('StudyInstanceUID', StudyInstanceUID);
    value = getValue(rdicom, instance, 0x00280010);
    const Rows = fromF64(rdicom, value);
    console.log('Rows', Rows);
    value = getValue(rdicom, instance, 0x00280011);
    const Columns = fromF64(rdicom, value);
    console.log('Columns', Columns);
    window.instance = instance;
    value = getValue(rdicom, instance, 0x7fe00010);
    const pixels = fromArrayBuffer(rdicom, value);
    console.log(pixels.length, pixels);
    const imageCanvas = document.createElement('canvas');
    imageCanvas.width = Columns;
    imageCanvas.height = Rows;
    console.log(`${Columns}x${Rows}`);
    const imageCtx = imageCanvas.getContext('2d');
    const imageData = imageCtx.getImageData(0, 0, imageCanvas.width, imageCanvas.height);
    console.log('imageData.data.length / 4', imageData.data.length / 4);
    for (let i = 0; i < imageData.data.length / 4; ++i) {
      imageData.data[i * 4    ] = pixels[i * 3    ];
      imageData.data[i * 4 + 1] = pixels[i * 3 + 1];
      imageData.data[i * 4 + 2] = pixels[i * 3 + 2];
      imageData.data[i * 4 + 3] = 0xFF;
    }
    imageCtx.putImageData(imageData, 0, 0);
    const ctx = canvas.getContext('2d');
    if (Columns / Rows > canvas.width / canvas.height) {
      // console.log(0, Math.round(canvas.height / 2) - Math.round(Rows / 2));
      // ctx.drawImage(imageCanvas, 0, Math.round(canvas.height / 2) - Math.round(Rows / 2));
      const zoom = canvas.width / Columns;
      ctx.setTransform(
        zoom, 0, 0, zoom, 0,
        Math.round(canvas.height / 2) - Math.round(zoom * Rows / 2),
      );
    } else {
      const zoom = canvas.height / Rows;
      ctx.setTransform(
        zoom, 0, 0, zoom,
        Math.round(canvas.width / 2) - Math.round(zoom * Columns / 2), 0,
      );
    }
    ctx.drawImage(imageCanvas, 0, 0);
  });
}

window.onload = main;

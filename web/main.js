// https://surma.dev/things/rust-to-webassembly/

const textDecoder = new TextDecoder();

// From a position in a buffer, assume a null terminated c-string and return
// a javascript string.
function toStr(charArray, ptr, limit=255) {
  let end = ptr;
  while (charArray[end++] && (end - ptr) < limit);
  return textDecoder.decode(new Uint8Array(charArray.buffer, ptr, end - ptr - 1));
}

async function rdicomInit(rdicom_path) {
  // By default, memory is 1 page (64K). We'll need a little more
  const memory = new WebAssembly.Memory({ initial: 1000 });
  console.log(memory.buffer.byteLength / 1024, 'KB allocated');

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
    // libc memset reimplementation
    memset: (ptr, value, size) => {
      console.log('memset');
      const mem = new Uint8Array(memory.buffer);
      mem.fill(value, ptr, ptr + size);
      return ptr;
    },
    // libc memcpy reimplementation
    memcpy: (dest, source, n) => {
      console.log('memcpy');
      const mem = new Uint8Array(memory.buffer);
      mem.copyWithin(dest, source, source + n);
      return dest;
    },
    // libc memcmp reimplmentation
    memcmp: (s1, s2, n) => {
      console.log('memcmp');
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
    malloc: size => {
      const ptr = heapPos;
      heapPos += size;
      console.log('malloc', size, `-> 0x${ptr.toString(16)}`);
      return ptr;
    },
    // libc free reimplementation
    free: ptr => {
      console.log('free');
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

  console.log('rdicom loaded')
  return rdicom;
}

const fileReader = new FileReader();

function setupCanvas(id, useBuffer) {
  const canvas = document.getElementById(id);
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
    useBuffer(buffer);
  });
}

function getInstanceFromBuffer(rdicom, buffer) {
  const { instance_from_ptr } = rdicom.instance.exports;
  const ptr = rdicom.env.malloc(buffer.byteLength);
  // const memory = new Uint8Array(rdicom.env.memory.buffer);
  // memory.set(new Uint8Array(buffer), ptr);
  console.log('ptr', ptr);
  const handle = instance_from_ptr(ptr, buffer.byteLength);
  return handle;
}

async function main() {
  const rdicom = await rdicomInit('rdicom.wasm');
  let instance; // the DICOM instance
  setupCanvas('vp1', buffer => {
    instance = getInstanceFromBuffer(rdicom, buffer);
    console.log('instance loaded');
  });
}

window.onload = main;
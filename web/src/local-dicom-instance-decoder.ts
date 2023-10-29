// We are importing the release build of rdicom.wasm. Make sure that the latest
// version has been compiled before building the frontend component.
import rdicomcode from '../../target/wasm32-unknown-unknown/release/rdicom.wasm';

export type InstanceHandle = number;

export class LocalDicomInstanceDecoder {
  memory: WebAssembly.Memory;
  textDecoder = new TextDecoder();
  rdicom?: WebAssembly.WebAssemblyInstantiatedSource;
  runtime: any; // TODO: need to define a type here.

  /**
   * Constructs a LocalDicomInstanceDecoder
   * @param {number = 1000} nbpages Number of 64K pages to be allocated (Default to 64MB).
   */
  constructor(nbpages: number = 1000) {
    // By default, memory is 1 page (64K). We'll need a little more
    this.memory = new WebAssembly.Memory({ initial: nbpages });
  }

  get<T>(instanceHandle: InstanceHandle, tag: number, tagtype: string): T {
    if (this.rdicom === undefined) {
      throw new Error('LocalDicomInstanceDecoder not properly initialized. rdicom is undefined.');
    }
    switch (tagtype) {
      case 'number': {
        const value = this.getValue(this.rdicom, instanceHandle, tag);
        return this.fromF64(value) as any;
      }
      case 'Uint8Array': {
        const value = this.getValue(this.rdicom, instanceHandle, tag);
        return this.fromArrayBuffer(value) as any;
      }
      case 'string': {
        const value = this.getValue(this.rdicom, instanceHandle, tag);
        return this.fromCString(value) as any;
      }
      case 'Float32Array':
      case 'Array<Array<string> | undefined>':
      case 'Array<Date | undefined>':
      case 'Array<number | undefined>':
      case 'Array<Partial<Dataset> | undefined>':
      case 'Array<string>':
      case 'Array<string | undefined>':
      case 'Array<Uint16Array | undefined>':
      case 'Array<Uint8Array | undefined>':
      case 'Date':
      case 'Float64Array':
      case 'Uint16Array':
      case 'Uint32Array':
      case 'Array<any | undefined>':
      case 'any':
      case 'undefined':
      default:
    }

    throw new Error(`LocalDicomInstanceDecoder: unsupported type ${tagtype} for tag ${tag}`);
  }

  getInstanceFromBuffer(buffer: ArrayBuffer): InstanceHandle {
    if (this.rdicom === undefined) {
      throw new Error('LocalDicomInstanceDecoder not properly initialized (rdicom is undefined)');
    }
    const { instance_from_ptr } = this.rdicom.instance.exports as {
      instance_from_ptr: (ptr: number, size: number) => InstanceHandle,
    };
    // Allocate memory and ptr points to index at which the allocated buffer starts
    const ptr = this.runtime.malloc(buffer.byteLength);
    // Map the whole wasm memory
    const memory = new Uint8Array(this.memory.buffer);
    // Set the content of the DICOM file to the wasm memory at index ptr
    memory.set(new Uint8Array(buffer), ptr);
    const handle = instance_from_ptr(ptr, buffer.byteLength);
    return handle;
  }

  private getValue(rdicom: WebAssembly.WebAssemblyInstantiatedSource, instance: InstanceHandle,
    tag: number): number {
    const { get_value_from_ptr } = rdicom.instance.exports as {
      get_value_from_ptr: (i: InstanceHandle, t: number) => number,
    };
    return get_value_from_ptr(instance, tag);
  }

  // From a position in a buffer, assume a null terminated c-string and return
  // a javascript string.
  toStr(charArray: Uint8Array, ptr: number, limit = 255): string {
    let end = ptr;
    while (charArray[end++] && (end - ptr) < limit);
    return this.textDecoder.decode(new Uint8Array(charArray.buffer, ptr, end - ptr - 1));
  }

  fromCString(offset: number): string {
    const memory = new Uint8Array(this.memory.buffer);
    let zero = offset;
    while (memory[zero] !== 0) zero++;
    return this.textDecoder.decode(new Uint8Array(memory.buffer, offset, zero - offset));
  }

  fromF64(offset: number): number {
    const float64 = new Float64Array(this.memory.buffer, offset)
    return float64[0];
  }

  fromArrayBuffer(offset: number): Uint8Array {
    const vector = new Uint32Array(this.memory.buffer, offset);
    return new Uint8Array(this.memory.buffer, vector[1], vector[0]);
  }

  /**
   * Initializes the Decoder.
   * @param {string} rdicompath Path to the rdicom wasm code file to be fetched.
   * If not provided, the module will use the code embedded in the rdicom-web
   * module.
   */
  async init(rdicompath?: string): Promise<void> {
    const memory = this.memory;
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
      addString: (offset: number, size: number) => {
        console.log('offset', offset.toString(16), 'size', size);
        // window.view = new Uint8Array(memory.buffer);
        // for (let i = 0; i < view.length; ++i) {
        //   console.log(i, view[i]);
        // }
        str = str + this.textDecoder.decode(new Uint8Array(memory.buffer, offset, size));
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
      memset: (ptr: number, value: number, size: number) => {
        // console.log('memset');
        const mem = new Uint8Array(memory.buffer);
        mem.fill(value, ptr, ptr + size);
        return ptr;
      },
      // libc memcpy reimplementation
      memcpy: (dest: number, source: number, n: number) => {
        // console.log('memcpy');
        const mem = new Uint8Array(memory.buffer);
        mem.copyWithin(dest, source, source + n);
        return dest;
      },
      // libc memcmp reimplmentation
      memcmp: (s1: number, s2: number, n: number) => {
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
      malloc: (size: number, align = 1) => {
        let ptr = heapPos;
        heapPos += size;
        // Align ptr
        const mod = ptr % align;
        if (mod !== 0) {
          const move_to_align = align - (ptr % align);
          ptr += move_to_align;
          heapPos += move_to_align;
        }
        // console.log(`malloc(${size}, ${align})`, `-> 0x${ptr.toString(16)} (${ptr})`);
        return ptr;
      },
      // libc free reimplementation
      free: (ptr: number) => {
        // console.log(`free 0x${ptr.toString(16)} (${ptr})`);
        // Nothing gets freed
      },
      __assert_fail_js: (assertion: number, file: number, line: number, fun: number) => {
        const charArray = new Uint8Array(memory.buffer);
        console.log(`${this.toStr(charArray, file)}(${line}): ${this.toStr(charArray, assertion)} in ${this.toStr(charArray, fun)}`);
      },
    }
    // Load the wasm code
    // const rdicomcode = new Uint8Array();
    const rdicom = (rdicompath !== undefined)
      ? await WebAssembly.instantiateStreaming(fetch(rdicompath), { env })
      : await WebAssembly.instantiate(rdicomcode, { env });
    heapPos = (rdicom.instance.exports.__heap_base as WebAssembly.Global).value;
    this.rdicom = rdicom;
    this.runtime = env;
  }

  private toDate(datestr: string | undefined): Date | undefined {
    if (datestr === undefined) return undefined;
    const canondatestr = (datestr.includes('.'))
      ? datestr.split('.').join('') // ACR-NEMA Standard 300 date format
      : datestr;
    const year = Number(canondatestr.substr(0, 4));
    const month = Number(canondatestr.substr(4, 2));
    const day = Number(canondatestr.substr(6, 2));
    return new Date(year, month, day);
  }
}

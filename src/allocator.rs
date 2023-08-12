// Copyright (c) 2023 Jean-Daniel Michaud
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

// https://surma.dev/things/rust-to-webassembly/

use core::alloc::GlobalAlloc;
use core::alloc::Layout;

#[link(wasm_import_module = "env")]
extern "C" {
  fn malloc(size: usize, align: usize) -> *mut u8;
  fn free(ptr: *const u8);
}

#[repr(C, align(32))]
pub struct WasmAllocator {}

impl WasmAllocator {
  pub const fn new() -> Self {
    WasmAllocator {}
  }
}

unsafe impl Sync for WasmAllocator {}

unsafe impl GlobalAlloc for WasmAllocator {
  unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
    let size: usize = layout.size();
    let align: usize = layout.align();
    malloc(size, align)
  }

  unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
    free(ptr);
  }
}

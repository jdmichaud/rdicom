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

// Doc on rust and wasm: https://surma.dev/things/rust-to-webassembly/

#![feature(error_in_core)] // core::error:Error only available on nightly for now
#![cfg_attr(target_arch = "wasm32", no_std)] // If compiling for a wasm target, do not use no_std

#[macro_use]
extern crate alloc; // We need this in order to use alloc modules

#[cfg(target_arch = "wasm32")]
mod allocator;

// #[cfg(target_arch = "wasm32")]
// #[link(wasm_import_module = "env")]
// extern "C" {
//   static memory: *const u8;
// }

#[cfg(target_arch = "wasm32")]
#[global_allocator]
static ALLOCATOR: allocator::WasmAllocator = allocator::WasmAllocator::new();

#[cfg(target_arch = "wasm32")]
#[panic_handler]
fn panic(_panic: &core::panic::PanicInfo<'_>) -> ! {
  // if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
  //     console_log("panic occurred: {s:?}");
  // } else {
  //     console_log("panic occurred");
  // }
  core::arch::wasm32::unreachable()
}

pub mod dicom_tags;
pub mod error;
pub mod instance;
pub mod misc;
pub mod tags;

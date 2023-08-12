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

extern crate alloc; // We need this in order to use alloc modules

use alloc::vec::Vec;

use crate::instance::DicomValue;
use crate::instance::Instance;

/**
 * In wasm environment we will need to implement our own allocator which will
 * liaise with Javascript side functions to allocate/free memory.
 */
use crate::allocator;

#[global_allocator]
static ALLOCATOR: allocator::WasmAllocator = allocator::WasmAllocator::new();

// We need CString for writing null-terminated string in wasm memory.
use alloc::ffi::CString;

#[link(wasm_import_module = "env")]
extern "C" {
  fn addString(s: *const u8, len: usize);
  fn printString();
  fn printError();
}

fn console_log(s: &str) {
  unsafe {
    let c_str = CString::new(s).unwrap();
    addString(c_str.as_ptr() as *const u8, s.len());
    printString();
  }
}

fn console_error(s: &str) {
  unsafe {
    let c_str = CString::new(s).unwrap();
    addString(c_str.as_ptr() as *const u8, s.len());
    printError();
  }
}

#[panic_handler]
fn panic(panic_info: &core::panic::PanicInfo<'_>) -> ! {
  if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
    console_error("panic: {s:?}");
  }
  core::arch::wasm32::unreachable()
}

/**
 * Creates an instance from a buffer containing a DICOM file.
 */
#[no_mangle]
pub extern "C" fn instance_from_ptr(ptr: *mut u8, len: usize) -> *const Instance {
  let buffer = unsafe { Vec::from_raw_parts(ptr, len, len) };
  match Instance::from(buffer) {
    Ok(instance) => core::ptr::addr_of!(instance),
    Err(e) => {
      panic!("error while creating the instance");
    }
  }
}

fn dicom_value_to_memory(dicom_value: &DicomValue) -> *const i8 {
  match dicom_value {
    // DicomValue::AE(strings) |
    // DicomValue::AS(strings) |
    // DicomValue::CS(strings) |
    // DicomValue::DA(strings) |
    // DicomValue::DS(strings) |
    // DicomValue::DT(strings) |
    // DicomValue::IS(strings) |
    // DicomValue::LO(strings) |
    // DicomValue::LT(strings) |
    // DicomValue::PN(strings) |
    // DicomValue::SH(strings) |
    // DicomValue::ST(strings) |
    // DicomValue::TM(strings) |
    // DicomValue::UT(strings) => {
    //   let buffer_size: usize = strings.iter().reduce(|acc, value| acc += value.length).collect::<_>();
    //   let buffer = malloc(buffer_size + core::mem::size_of<usize>());
    //   let number_of_string_as_bytes = strings.length.to_le_bytes();
    //   core::ptr::copy_nonoverlapping(number_of_string_as_bytes, buffer, core::mem::size_of<usize>());
    // },
    DicomValue::UI(value) => {
      let c_str = CString::new(value.as_str()).unwrap();
      return c_str.into_raw();
    }
    _ => todo!(),
  }
}

/**
 * Gets a value from an instance and a tag as a 32 unsigned bits (e.g.: 0x0020000d)
 */
#[no_mangle]
pub extern "C" fn get_value_from_ptr(instance_ptr: *mut u8, tagid: u32) -> *const i8 {
  let instance: Instance = unsafe { core::ptr::read(instance_ptr as *const Instance) };
  let tag = &(tagid.try_into().expect("tag to exists"));
  let dicom_value = instance.get_value(&tag).expect("value to be read");
  let c_str;
  match dicom_value {
    Some(DicomValue::UI(value)) => {
      c_str = CString::new(value).unwrap();
      return c_str.into_raw();
    }
    None => {
      return core::ptr::null();
    }
    _ => todo!(),
  }
}

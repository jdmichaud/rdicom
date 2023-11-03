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

use crate::instance::DicomValue;
use crate::instance::Instance;
use crate::tags::Tag;

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
  if let Some(message) = panic_info.message() {
    if let Some(location) = panic_info.location() {
      console_error(&format!(
        "panic: {message} ({}:{}:{})",
        location.file(),
        location.line(),
        location.column()
      ));
    } else {
      console_error(&format!("panic: {message:?}"));
    }
  }
  core::arch::wasm32::unreachable()
}

/**
 * Creates an instance from a buffer containing a DICOM file.
 */
#[no_mangle]
pub extern "C" fn instance_from_ptr(ptr: *mut u8, len: usize) -> *const Instance {
  let buffer = unsafe { alloc::slice::from_raw_parts(ptr, len) };
  match Instance::from(buffer) {
    Ok(instance) => {
      let p_instance = alloc::boxed::Box::new(instance);
      alloc::boxed::Box::into_raw(p_instance)
    }
    Err(e) => {
      panic!("error: {e} while creating the instance");
    }
  }
}

/**
 * Gets a value from an instance and a tag as a 32 unsigned bits (e.g.: 0x0020000d)
 */
#[no_mangle]
pub extern "C" fn get_value_from_ptr(instance_ptr: *const Instance, tagid: u32) -> *const u8 {
  let instance: &Instance = unsafe { instance_ptr.as_ref().unwrap() };
  let tag: Tag = tagid.try_into().expect("tag to exists");
  let dicom_value = instance.get_value(&tag).unwrap(); // TODO: to something smarter here
  let res = match dicom_value {
    Some(value) => dicom_value_to_memory(&value),
    None => core::ptr::null(),
  };
  res
}

fn stream_number<T: Into<f64>>(value: T) -> *const u8 {
  let fvalue: f64 = value.into();
  let buffer: *mut u8 = unsafe { ALLOCATOR.alloc_t::<f64>(core::mem::size_of::<f64>()) };
  let number_of_string_as_bytes = fvalue.to_le_bytes();
  unsafe {
    core::ptr::copy_nonoverlapping(
      number_of_string_as_bytes.as_ptr(),
      buffer,
      core::mem::size_of::<f64>(),
    );
  }
  buffer
}

// Stream to memory the dicom value. We expect Javascript to be able to unpack
// those depending on the type of the Tag.
fn dicom_value_to_memory(dicom_value: &DicomValue) -> *const u8 {
  match dicom_value {
    DicomValue::AE(strings)
    | DicomValue::AS(strings)
    | DicomValue::CS(strings)
    | DicomValue::DA(strings)
    | DicomValue::DS(strings)
    | DicomValue::DT(strings)
    | DicomValue::IS(strings)
    | DicomValue::LO(strings)
    | DicomValue::LT(strings)
    | DicomValue::PN(strings)
    | DicomValue::SH(strings)
    | DicomValue::ST(strings)
    | DicomValue::TM(strings)
    | DicomValue::UT(strings) => {
      // Compute the size of all the string put together + a 4 bytes for each string length
      let buffer_size: usize = strings.iter().fold(0, |acc, value| acc + value.len() + 4);

      // The buffer will contain the number of strings and then, for each string,
      // the size of the string and a copy of the string without the terminating
      // null character
      let buffer = unsafe { ALLOCATOR.alloc_t::<usize>(buffer_size + core::mem::size_of::<u32>()) };
      let mut ptr = buffer;

      unsafe {
        // Prefix the buffer with the number of element in the vector
        let number_of_string: u32 = u32::try_from(strings.len()).unwrap();
        core::ptr::copy_nonoverlapping(
          number_of_string.to_le_bytes().as_ptr(),
          ptr,
          core::mem::size_of::<u32>(),
        );
        ptr = ptr.wrapping_add(core::mem::size_of::<u32>());
        // Then append the pointer to all the string
        for s in strings {
          let c_str = CString::new(s.clone()).unwrap();
          let ssize = u32::try_from(s.len()).unwrap().to_le_bytes();
          core::ptr::copy_nonoverlapping(ssize.as_ptr(), ptr, core::mem::size_of::<u32>());
          ptr = ptr.wrapping_add(core::mem::size_of::<u32>());
          // Do not copy the terminal null character
          core::ptr::copy_nonoverlapping(c_str.as_ptr(), ptr as *mut i8, s.len());
          ptr = ptr.wrapping_add(s.len());
        }
      }
      buffer
    }
    DicomValue::UI(value) => {
      let c_str = CString::new(value.as_str()).unwrap();
      return c_str.into_raw() as *const u8;
    }
    DicomValue::SL(value) => stream_number(*value),
    DicomValue::SS(value) => stream_number(*value),
    DicomValue::UL(value) => stream_number(*value),
    DicomValue::US(value) => stream_number(*value),
    DicomValue::FD(values) => {
      let buffer_size = values.len() * core::mem::size_of::<f64>();
      let buffer = unsafe { ALLOCATOR.alloc_t::<f64>(buffer_size) };
      unsafe {
        core::ptr::copy_nonoverlapping(values.as_ptr() as *const u8, buffer, buffer_size);
      }
      buffer
    }
    DicomValue::FL(values) => {
      let buffer_size = values.len() * core::mem::size_of::<f64>();
      let buffer = unsafe { ALLOCATOR.alloc_t::<f64>(buffer_size) };
      let fvalues = values
        .iter()
        .map(|&n| n as f64)
        .collect::<alloc::vec::Vec<f64>>();
      unsafe {
        core::ptr::copy_nonoverlapping(fvalues.as_ptr() as *const u8, buffer, buffer_size);
      }
      buffer
    }
    DicomValue::UN(values) | DicomValue::OB(values) => {
      // Allocate two u32, one for size of the buffer and one for its index (pointer)
      let buffer = unsafe { ALLOCATOR.alloc_t::<u32>(2) };
      let buffer_size = u32::try_from(values.len()).unwrap();
      let ptr = u32::try_from(values.as_ptr() as u64).unwrap();
      // TODO: We should check pointer size maybe
      let data: alloc::vec::Vec<u32> = vec![buffer_size, ptr];
      unsafe {
        core::ptr::copy_nonoverlapping(
          data.as_ptr() as *const u8,
          buffer,
          2 * core::mem::size_of::<u32>(),
        );
      }
      buffer
    }
    DicomValue::OW(values) => {
      // Allocate two u32, one for size of the buffer and one for its index (pointer)
      let buffer = unsafe { ALLOCATOR.alloc_t::<u32>(2) };
      // OW is 16bits so 2 bytes
      let buffer_size = u32::try_from(values.len() * 2).unwrap();
      let ptr = u32::try_from(values.as_ptr() as u64).unwrap();
      // TODO: We should check pointer size maybe
      let data: alloc::vec::Vec<u32> = vec![buffer_size, ptr];
      unsafe {
        core::ptr::copy_nonoverlapping(
          data.as_ptr() as *const u8,
          buffer,
          2 * core::mem::size_of::<u32>(),
        );
      }
      buffer
    }
    DicomValue::SeqEnd | DicomValue::SeqItemEnd => core::ptr::null(),
    DicomValue::AT(_)
    | DicomValue::SQ(_)
    | DicomValue::OL(_)
    | DicomValue::OV(_)
    | DicomValue::OF(_)
    | DicomValue::OD(_)
    | DicomValue::SV(_)
    | DicomValue::UC(_)
    | DicomValue::UR(_)
    | DicomValue::UV(_)
    | DicomValue::SeqItem(_) => unimplemented!(),
  }
}

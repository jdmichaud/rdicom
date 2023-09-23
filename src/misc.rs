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

use alloc::vec::Vec;
use core::str::from_utf8;

pub fn has_dicom_header(buffer: &[u8]) -> bool {
  let _d = buffer[0x80];
  let _i = buffer[0x81];
  let _c = buffer[0x82];
  let _o = buffer[0x83];
  let _m = buffer[0x84];
  buffer.len() > 0x84 && from_utf8(&buffer[0x80..0x80 + 4]) == Ok("DICM")
}

/**
 * Check if a file is a DICOM file.
 * Imperfect heuristic for now.
 */
#[cfg(not(target_arch = "wasm32"))]
pub fn is_dicom_file(file_path: &str) -> bool {
  match std::fs::read(file_path) {
    Ok(buf) => is_dicom(&buf),
    Err(_) => false,
  }
}

pub fn is_dicom(buffer: &Vec<u8>) -> bool {
  has_dicom_header(buffer)
}

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

use core::array::TryFromSliceError;
use core::str::Utf8Error;
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub struct DicomError {
  pub details: String,
}

impl DicomError {
  pub fn new(msg: &str) -> DicomError {
    DicomError {
      details: msg.to_string(),
    }
  }
}

impl fmt::Display for DicomError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{}", self.details)
  }
}

impl Error for DicomError {
  fn description(&self) -> &str {
    &self.details
  }
}

impl From<Box<dyn std::error::Error>> for DicomError {
  fn from(err: Box<dyn std::error::Error>) -> Self {
    // TODO: Improve this...
    DicomError::new(&format!("{:?}", err))
  }
}

impl From<Utf8Error> for DicomError {
  fn from(err: Utf8Error) -> Self {
    match err.error_len() {
      Some(l) => DicomError::new(&format!(
        "UTF8 error: an unexpected byte was encountered at {}",
        l
      )),
      None => DicomError::new("UTF8 error: the end of the input was reached unexpectedly"),
    }
  }
}

impl From<TryFromSliceError> for DicomError {
  fn from(err: TryFromSliceError) -> Self {
    // TODO: Improve this...
    DicomError::new(&format!("{:?}", err))
  }
}

impl From<std::io::Error> for DicomError {
  fn from(err: std::io::Error) -> Self {
    // TODO: Improve this...
    DicomError::new(&format!("{:?}", err))
  }
}

impl From<std::num::ParseIntError> for DicomError {
  fn from(err: std::num::ParseIntError) -> Self {
    // TODO: Improve this...
    DicomError::new(&format!("{:?}", err))
  }
}

impl From<&str> for DicomError {
  fn from(err: &str) -> Self {
    DicomError::new(err)
  }
}

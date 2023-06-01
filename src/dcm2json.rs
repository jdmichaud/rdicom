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

#![allow(dead_code)]
#![allow(unused_variables)]

use std::error::Error;
use std::fs::File;
use structopt::clap::AppSettings;
use structopt::StructOpt;

mod dicom_representation;

use crate::dicom_representation::dcm2json;
use rdicom::error::DicomError;
use rdicom::misc::is_dicom_file;

// A simplified dcm2json clone
#[derive(Debug, StructOpt)]
#[structopt(
  name = format!("dcm2json {} ({} {})", env!("GIT_HASH"), env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")),
  no_version,
  global_settings = &[AppSettings::DisableVersion]
)]
struct Opt {
  /// DICOM input file to be converted to JSON
  filepath: String,
}

fn main() -> Result<(), Box<dyn Error>> {
  let opt = Opt::from_args();
  let f = File::open(&opt.filepath)?;
  let result: Result<_, Box<dyn Error>> = if is_dicom_file(&opt.filepath) {
    dcm2json(f)
  } else {
    Err(Box::new(DicomError::new(&format!(
      "{} is not a dicom file",
      opt.filepath
    ))))
  };

  match result {
    Ok(result) => println!("{}", serde_json::to_string(&result)?),
    Err(e) => eprintln!("error: {}", e),
  }
  Ok(())
}

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

use rdicom::dicom_representation::{json2dcm, DicomAttributeJson};
use rdicom::error::DicomError;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufReader;
use std::io::BufWriter;
use structopt::clap::AppSettings;
use structopt::StructOpt;

// A simplified dcm2json clone
#[derive(Debug, StructOpt)]
#[structopt(
  name = format!("json2dcm {} ({} {})", env!("GIT_HASH"), env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")),
  no_version,
  global_settings = &[AppSettings::DisableVersion]
)]
struct Opt {
  /// DICOM Json input file
  jsonfilepath: String,
  /// DICOM binary output file
  dcmfilepath: String,
}

fn main() -> Result<(), DicomError> {
  let opt = Opt::from_args();
  let inputfile = File::open(&opt.jsonfilepath)?;
  let json: BTreeMap<String, DicomAttributeJson> =
    serde_json::from_reader(BufReader::new(inputfile)).unwrap();

  let outputfile = File::create(&opt.dcmfilepath)?;
  let mut writer = BufWriter::new(outputfile);
  json2dcm::json2dcm(&mut writer, &json)
}

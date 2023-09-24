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
#![allow(unused_imports)]

use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::io::{self};

use structopt::StructOpt;

use rdicom::dicom_tags::RequestedProcedureID;
use rdicom::dicom_tags::SequenceDelimitationItem;
use rdicom::instance::DicomValue;
use rdicom::instance::Instance;
use rdicom::misc::is_dicom_file;

#[derive(Debug, StructOpt)]
struct Opt {
  filepath: String,
}

fn main() -> Result<(), Box<dyn Error>> {
  let opt = Opt::from_args();
  let f = File::open(&opt.filepath)?;

  if is_dicom_file(&opt.filepath) {
    let instance = Instance::from_buf_reader(BufReader::new(f))?;
    // println!("{:?}", instance.get_value(&0x00082112.try_into()?)?);
    // println!("{:?}", instance.get_value(&0x0008114A.try_into()?)?);
    // println!("{:?}", instance.get_value(&0x00081250.try_into()?)?);
    // println!("{:?}", instance.get_value(&0x7FE00000.try_into()?)?);
    // println!("{:?}", instance.get_value(&RequestedProcedureID)?);
    println!("{:?}", instance.get_value(&rdicom::dicom_tags::PatientName));
  } else {
    println!("error: {} unrecognized dicom file", opt.filepath);
  }
  Ok(())
}

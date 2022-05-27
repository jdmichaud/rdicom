#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]

use std::error::Error;
use std::io::BufReader;
use std::fs::File;
use std::io::{self};

use structopt::StructOpt;

use rdicom::instance::Instance;
use rdicom::misc::is_dicom_file;
use rdicom::instance::DicomValue;
use rdicom::dicom_tags::SequenceDelimitationItem;

#[derive(Debug, StructOpt)]
struct Opt {
  filepath: String,
}

fn main() -> Result<(), Box<dyn Error>> {
  let opt = Opt::from_args();
  let f = File::open(&opt.filepath)?;

  if is_dicom_file(&opt.filepath) {
    let instance = Instance::from_buf_reader(BufReader::new(f))?;
    println!("{:?}", instance.get_value(&0x0008114A.into()));
  }
  Ok(())
}

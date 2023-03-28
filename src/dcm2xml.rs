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

use rdicom::instance::DicomValue;
use std::fs::File;
use std::io::BufReader;
use rdicom::misc::is_dicom_file;
use rdicom::instance::Instance;
use std::error::Error;
use rdicom::dicom_representation;
use rdicom::dicom_representation::{NativeDicomModel, ValueRepresentation};
use rdicom::error::DicomError;
use structopt::StructOpt;
use base64::{Engine as _, engine::general_purpose};

#[derive(Debug, StructOpt)]
struct Opt {
  /// DICOM input file to be converted to XML
  filepath: String,
}

// TODO: Ugly; replace vr: &str in DicomAttribute with ValueRepresentation
fn vr_to_vr(vr: &str) -> Result<ValueRepresentation, DicomError> {
  match vr {
    "AE" => Ok(ValueRepresentation::AE),
    "AS" => Ok(ValueRepresentation::AS),
    "AT" => Ok(ValueRepresentation::AT),
    "CS" => Ok(ValueRepresentation::CS),
    "DA" => Ok(ValueRepresentation::DA),
    "DS" => Ok(ValueRepresentation::DS),
    "DT" => Ok(ValueRepresentation::DT),
    "FL" => Ok(ValueRepresentation::FL),
    "FD" => Ok(ValueRepresentation::FD),
    "IS" => Ok(ValueRepresentation::IS),
    "LO" => Ok(ValueRepresentation::LO),
    "LT" => Ok(ValueRepresentation::LT),
    "OB" => Ok(ValueRepresentation::OB),
    "OD" => Ok(ValueRepresentation::OD),
    "OF" => Ok(ValueRepresentation::OF),
    "OL" => Ok(ValueRepresentation::OL),
    "OV" => Ok(ValueRepresentation::OV),
    "OW" => Ok(ValueRepresentation::OW),
    "PN" => Ok(ValueRepresentation::PN),
    "SH" => Ok(ValueRepresentation::SH),
    "SL" => Ok(ValueRepresentation::SL),
    "SQ" => Ok(ValueRepresentation::SQ),
    "SS" => Ok(ValueRepresentation::SS),
    "ST" => Ok(ValueRepresentation::ST),
    "SV" => Ok(ValueRepresentation::SV),
    "TM" => Ok(ValueRepresentation::TM),
    "UC" => Ok(ValueRepresentation::UC),
    "UI" => Ok(ValueRepresentation::UI),
    "UL" => Ok(ValueRepresentation::UL),
    "UN" => Ok(ValueRepresentation::UN),
    "UR" => Ok(ValueRepresentation::UR),
    "US" => Ok(ValueRepresentation::US),
    "UT" => Ok(ValueRepresentation::UT),
    "UV" => Ok(ValueRepresentation::UV),
    _ => Err(DicomError::new(&format!("Unknown value representation {}", vr)))
  }
}

fn dcm2nativexml(f: File) -> Result<NativeDicomModel, Box<dyn Error>> {
  let instance = Instance::from_buf_reader(BufReader::new(f))?;
  let mut dicom_attributes: Vec<dicom_representation::DicomAttribute> = vec![];
  for dicom_attribute in instance.iter() {
    let dicom_attribute = dicom_attribute?;
    let dicom_value = DicomValue::from_dicom_attribute(&dicom_attribute, &instance)?;
    let value = match dicom_value {
      DicomValue::OB(content) => general_purpose::STANDARD.encode(content),
      DicomValue::OW(content) => {
        let content8: &[u8] = unsafe {
          std::slice::from_raw_parts(content.as_ptr() as *const u8, content.len() / 2)
        };
        general_purpose::STANDARD.encode(content8)
      },
      _ => dicom_value.to_string(),
    };
    dicom_attributes.push(dicom_representation::DicomAttribute {
      tag: format!("{:04}{:04}", dicom_attribute.tag.group, dicom_attribute.tag.element),
      vr: vr_to_vr(&dicom_attribute.vr)?,
      keyword: Some(dicom_attribute.tag.name.to_string()),
      private_creator: None,
      payload: Some(dicom_representation::Payload::Value(vec![value])),
    });
  }
  Ok(NativeDicomModel { dicom_attributes: dicom_attributes })
}

fn main() -> Result<(), Box<dyn Error>> {
  let opt = Opt::from_args();
  let f = File::open(&opt.filepath)?;
  let result: Result<NativeDicomModel, Box<dyn Error>> = if is_dicom_file(&opt.filepath) {
    dcm2nativexml(f)
  } else {
    Err(Box::new(DicomError::new(&format!("{} is not a dicom file", opt.filepath))))
  };

  match result {
    Ok(result) => println!("{}", quick_xml::se::to_string(&result)?),
    Err(e) => eprintln!("error: {}", e),
  }
  Ok(())
}

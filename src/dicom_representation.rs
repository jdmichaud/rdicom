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

// https://dicom.nema.org/medical/dicom/current/output/chtml/part19/chapter_a.html#sect_A.1.6
// dcm2xml --native-format <dcmfile>

use crate::instance::DicomValue;
use std::io::BufReader;
use crate::instance::Instance;
use std::error::Error;
use std::fs::File;
use crate::error::DicomError;
use serde::{Deserialize, Serialize};
use base64::{Engine as _, engine::general_purpose};

#[derive(Debug, Serialize, Deserialize)]
pub enum ValueRepresentation {
  AE, AS, AT, CS, DA, DS, DT, FL, FD, IS, LO, LT, OB, OD, OF, OL, OV, OW, PN,
  SH, SL, SQ, SS, ST, SV, TM, UC, UI, UL, UN, UR, US, UT, UV,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Link {
  #[serde(rename = "@uuid")]
  UUID(String),
  #[serde(rename = "@uri")]
  URI(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Bulkdata {
  #[serde(flatten)]
  link: Link,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NameComponents {
  #[serde(rename = "FamilyName", skip_serializing_if = "Option::is_none")]
  family_name: Option<String>,
  #[serde(rename = "GivenName", skip_serializing_if = "Option::is_none")]
  given_name: Option<String>,
  #[serde(rename = "MiddleName", skip_serializing_if = "Option::is_none")]
  middle_name: Option<String>,
  #[serde(rename = "NamePrefix", skip_serializing_if = "Option::is_none")]
  name_prefix: Option<String>,
  #[serde(rename = "NameSuffix", skip_serializing_if = "Option::is_none")]
  name_suffix: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PersonName {
  Alphabetic(NameComponents),
  Phonetic(NameComponents),
  Ideographic(NameComponents),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Payload {
  Value(Vec<String>),
  BulkData(Bulkdata),
  // #[serde(rename = "Item"))]
  // item: Vec<DicomAttribute>,
  #[serde(rename = "PersonName")]
  PersonName(PersonName),
  #[serde(rename = "InlineBinary")]
  InlineBinary(String), // base64
}

// How to deal with mutually exclusive fields in serde https://stackoverflow.com/a/73604693/2603925
#[derive(Debug, Serialize, Deserialize)]
pub struct DicomAttribute {
  #[serde(rename = "@tag")]
  pub tag: String,
  #[serde(rename = "@vr")]
  pub vr: ValueRepresentation,
  #[serde(rename = "@keyword", skip_serializing_if = "Option::is_none")]
  pub keyword: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub private_creator: Option<String>,
  #[serde(flatten, skip_serializing_if = "Option::is_none")]
  pub payload: Option<Payload>,
}

// We need a special object for json because
// 1. quick-xml need to rename fields parsed in attribute with @
// 2. the json structure is different from the xml one, json streets the list
//    of attributes as an object not an array and the `tag` field is not a field
//    but a key.
#[derive(Debug, Serialize, Deserialize)]
pub struct DicomAttributeJson {
  #[serde(rename = "vr")]
  pub vr: ValueRepresentation,
  #[serde(rename = "keyword", skip_serializing_if = "Option::is_none")]
  pub keyword: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub private_creator: Option<String>,
  #[serde(flatten, skip_serializing_if = "Option::is_none")]
  pub payload: Option<Payload>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NativeDicomModel {
  #[serde(rename = "DicomAttribute")]
  pub dicom_attributes: Vec<DicomAttribute>,
}

// TODO: Ugly; replace vr: &str in DicomAttribute with ValueRepresentation
pub fn vr_to_vr(vr: &str) -> Result<ValueRepresentation, DicomError> {
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

pub fn dcm2native_dicom_model(f: File) -> Result<NativeDicomModel, Box<dyn Error>> {
  let instance = Instance::from_buf_reader(BufReader::new(f))?;
  let mut dicom_attributes: Vec<DicomAttribute> = vec![];
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
    dicom_attributes.push(DicomAttribute {
      tag: format!("{:04}{:04}", dicom_attribute.tag.group, dicom_attribute.tag.element),
      vr: vr_to_vr(&dicom_attribute.vr)?,
      keyword: Some(dicom_attribute.tag.name.to_string()),
      private_creator: None,
      payload: Some(Payload::Value(vec![value])),
    });
  }
  Ok(NativeDicomModel { dicom_attributes: dicom_attributes })
}

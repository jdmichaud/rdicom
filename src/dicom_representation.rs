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

use std::collections::BTreeMap;
use crate::instance::DicomValue;
use std::io::BufReader;
use crate::instance;
use crate::instance::Instance;
use std::error::Error;
use std::fs::File;
use crate::error::DicomError;
use serde::{Deserialize, Serialize};
use base64::{Engine as _, engine::general_purpose};

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub enum ValueRepresentation {
  AE, AS, AT, CS, DA, DS, DT, FL, FD, IS, LO, LT, OB, OD, OF, OL, OV, OW, PN,
  SH, SL, SQ, SS, ST, SV, TM, UC, UI, UL, UN, UR, US, UT, UV,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Link {
  #[serde(rename = "@uuid")]
  UUID(String),
  #[serde(rename = "@uri")]
  URI(String),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Bulkdata {
  #[serde(flatten)]
  link: Link,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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

// We need this Variant because, according to DICOM, in JSON the name is just
// a string whereas in XML it can be a structure (see NameComponents).
// Note that we accept both representation in both format even if DICOM consider
// this non-comformant.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum NameVariant {
  Name(String),
  NameComponents(NameComponents),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum PersonName {
  Alphabetic(NameVariant),
  Phonetic(NameVariant),
  Ideographic(NameVariant),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum ValuePayload {
  String(String),
  Numeral(f64),
  // DICOM is a mess. In XML, PatientName is an attribute with a field `PersonName`
  // and as such is part of the Payload of the attribute (see Payload) but in JSON
  // it is encoded as a Value. So we need to manage that case here.
  PersonName(PersonName),
  // Here again, sequence tag in JSON will be present in a Value object. But in
  // XML the format is different, the sequence will be below the DicomAttribute
  // tag directly enclosed in "Item" tags.
  Sequence(BTreeMap<String, DicomAttributeJson>),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Payload {
  Value(Vec<ValuePayload>),
  BulkData(Bulkdata),
  // #[serde(deserialize_with = "items_from_xml")]
  Item(Vec<DicomAttribute>), // Sequences will be here in XML
  PersonName(PersonName),
  InlineBinary(String), // base64
}

// How to deal with mutually exclusive fields in serde https://stackoverflow.com/a/73604693/2603925
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DicomAttribute {
  #[serde(rename = "@tag")]
  pub tag: String,
  #[serde(rename = "@vr")]
  pub vr: ValueRepresentation,
  #[serde(rename = "@keyword", skip_serializing_if = "Option::is_none")]
  pub keyword: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub private_creator: Option<String>,
  // payload is an option because, for example, IS (Integer String) attribute
  // can be empty string which must not translate to 0 but to an empty payload
  // At least, this behavior seems consistent between JSON and XML...
  #[serde(flatten, skip_serializing_if = "Option::is_none")]
  pub payload: Option<Payload>,
}

// We need a special object for json because
// 1. quick-xml need to rename fields parsed in attribute with @
// 2. the json structure is different from the xml one, json streams the list
//    of attributes as an object not an array and the `tag` field is not a field
//    but a key.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DicomAttributeJson {
  #[serde(rename = "vr")]
  pub vr: ValueRepresentation,
  #[serde(rename = "keyword", skip_serializing_if = "Option::is_none")]
  pub keyword: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub private_creator: Option<String>,
  // payload is an option because, for example, IS (Integer String) attribute
  // can be empty string which must not translate to 0 but to an empty payload
  // At least, this behavior seems consistent between JSON and XML...
  #[serde(flatten, skip_serializing_if = "Option::is_none")]
  pub payload: Option<Payload>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NativeDicomModel {
  #[serde(rename = "DicomAttribute")]
  pub dicom_attributes: Vec<DicomAttribute>,
}

impl TryFrom<&Payload> for String {
  type Error = DicomError;

  fn try_from(payload: &Payload) -> Result<Self, Self::Error> {
    match payload {
      Payload::Value(value) if value.len() == 1 => match &value[0] {
        ValuePayload::String(string_value) => Ok(string_value.clone()),
        ValuePayload::Numeral(numeral_value) => Ok(numeral_value.to_string()),
        ValuePayload::PersonName(PersonName::Alphabetic(NameVariant::Name(string_value))) => Ok(string_value.clone()),
        _ => Err(DicomError::new(&format!("Payload {:?} cannot be converted to a String", payload))),
      },
      Payload::Value(vec) =>
        Ok(vec.iter()
          .map(|entry| match entry {
            ValuePayload::String(s) => s.clone(),
            ValuePayload::Numeral(n) => { println!("{}", n); n.to_string() },
            _ => todo!(),
          })
          .collect::<Vec<String>>()
          .join("\\")),
      _ => Err(DicomError::new(&format!("Payload {:?} cannot be converted to a String", payload))),
    }
  }
}

impl TryFrom<Payload> for String {
  type Error = DicomError;

  fn try_from(payload: Payload) -> Result<Self, Self::Error> {
    (&payload).try_into()
  }
}

impl TryFrom<Payload> for u16 {
  type Error = DicomError;

  fn try_from(payload: Payload) -> Result<Self, Self::Error> {
    match payload {
      Payload::Value(value) => match &value[0] {
        ValuePayload::Numeral(u16_value) => Ok(*u16_value as u16),
        _ => Err(DicomError::new("Payload is not a u16")),
      },
      _ => Err(DicomError::new("Payload is not a u16")),
    }
  }
}

impl TryFrom<Payload> for i16 {
  type Error = DicomError;

  fn try_from(payload: Payload) -> Result<Self, Self::Error> {
    match payload {
      Payload::Value(value) => match &value[0] {
        ValuePayload::Numeral(i16_value) => Ok(*i16_value as i16),
        _ => Err(DicomError::new("Payload is not a i16")),
      },
      _ => Err(DicomError::new("Payload is not a i16")),
    }
  }
}

impl TryFrom<Payload> for u32 {
  type Error = DicomError;

  fn try_from(payload: Payload) -> Result<Self, Self::Error> {
    match payload {
      Payload::Value(value) => match &value[0] {
        ValuePayload::Numeral(u32_value) => Ok(*u32_value as u32),
        _ => Err(DicomError::new("Payload is not a u32")),
      },
      _ => Err(DicomError::new("Payload is not a u32")),
    }
  }
}

impl TryFrom<Payload> for i32 {
  type Error = DicomError;

  fn try_from(payload: Payload) -> Result<Self, Self::Error> {
    match payload {
      Payload::Value(value) => match &value[0] {
        ValuePayload::Numeral(i32_value) => Ok(*i32_value as i32),
        _ => Err(DicomError::new("Payload is not a i32")),
      },
      _ => Err(DicomError::new("Payload is not a i32")),
    }
  }
}

impl TryFrom<Payload> for f32 {
  type Error = DicomError;

  fn try_from(payload: Payload) -> Result<Self, Self::Error> {
    match payload {
      Payload::Value(value) => match &value[0] {
        ValuePayload::Numeral(f32_value) => Ok(*f32_value as f32),
        _ => Err(DicomError::new("Payload is not a f32")),
      },
      _ => Err(DicomError::new("Payload is not a f32")),
    }
  }
}

impl TryFrom<Payload> for f64 {
  type Error = DicomError;

  fn try_from(payload: Payload) -> Result<Self, Self::Error> {
    match payload {
      Payload::Value(value) => match &value[0] {
        ValuePayload::Numeral(f64_value) => Ok(*f64_value as f64),
        _ => Err(DicomError::new("Payload is not a f64")),
      },
      _ => Err(DicomError::new("Payload is not a f64")),
    }
  }
}

impl TryFrom<Payload> for Vec<u8> {
  type Error = DicomError;

  fn try_from(payload: Payload) -> Result<Self, Self::Error> {
    match payload {
      Payload::InlineBinary(content) => {
        let result = general_purpose::STANDARD.decode(content)
          .map_err(|_e| DicomError::new("error while decoding base64"))?;
        Ok(result)
      },
      _ => Err(DicomError::new("Payload cannot be converted to &[u8]")),
    }
  }
}

// https://stackoverflow.com/a/75303146/2603925
// TODO: Ugly; replace vr: &str in DicomAttribute with ValueRepresentation
impl From<ValueRepresentation> for &'static str {
  fn from(value_representation: ValueRepresentation) -> Self {
    match value_representation {
      ValueRepresentation::AE => "AE",
      ValueRepresentation::AS => "AS",
      ValueRepresentation::AT => "AT",
      ValueRepresentation::CS => "CS",
      ValueRepresentation::DA => "DA",
      ValueRepresentation::DS => "DS",
      ValueRepresentation::DT => "DT",
      ValueRepresentation::FL => "FL",
      ValueRepresentation::FD => "FD",
      ValueRepresentation::IS => "IS",
      ValueRepresentation::LO => "LO",
      ValueRepresentation::LT => "LT",
      ValueRepresentation::OB => "OB",
      ValueRepresentation::OD => "OD",
      ValueRepresentation::OF => "OF",
      ValueRepresentation::OL => "OL",
      ValueRepresentation::OV => "OV",
      ValueRepresentation::OW => "OW",
      ValueRepresentation::PN => "PN",
      ValueRepresentation::SH => "SH",
      ValueRepresentation::SL => "SL",
      ValueRepresentation::SQ => "SQ",
      ValueRepresentation::SS => "SS",
      ValueRepresentation::ST => "ST",
      ValueRepresentation::SV => "SV",
      ValueRepresentation::TM => "TM",
      ValueRepresentation::UC => "UC",
      ValueRepresentation::UI => "UI",
      ValueRepresentation::UL => "UL",
      ValueRepresentation::UN => "UN",
      ValueRepresentation::UR => "UR",
      ValueRepresentation::US => "US",
      ValueRepresentation::UT => "UT",
      ValueRepresentation::UV => "UV",
    }
  }
}

// TODO: Ugly; replace vr: &str in DicomAttribute with ValueRepresentation
impl<'a> From<&'a str> for ValueRepresentation {
  fn from(s: &'a str) -> Self {
    match s {
      "AE" => ValueRepresentation::AE,
      "AS" => ValueRepresentation::AS,
      "AT" => ValueRepresentation::AT,
      "CS" => ValueRepresentation::CS,
      "DA" => ValueRepresentation::DA,
      "DS" => ValueRepresentation::DS,
      "DT" => ValueRepresentation::DT,
      "FL" => ValueRepresentation::FL,
      "FD" => ValueRepresentation::FD,
      "IS" => ValueRepresentation::IS,
      "LO" => ValueRepresentation::LO,
      "LT" => ValueRepresentation::LT,
      "OB" => ValueRepresentation::OB,
      "OD" => ValueRepresentation::OD,
      "OF" => ValueRepresentation::OF,
      "OL" => ValueRepresentation::OL,
      "OV" => ValueRepresentation::OV,
      "OW" => ValueRepresentation::OW,
      "PN" => ValueRepresentation::PN,
      "SH" => ValueRepresentation::SH,
      "SL" => ValueRepresentation::SL,
      "SQ" => ValueRepresentation::SQ,
      "SS" => ValueRepresentation::SS,
      "ST" => ValueRepresentation::ST,
      "SV" => ValueRepresentation::SV,
      "TM" => ValueRepresentation::TM,
      "UC" => ValueRepresentation::UC,
      "UI" => ValueRepresentation::UI,
      "UL" => ValueRepresentation::UL,
      "UN" => ValueRepresentation::UN,
      "UR" => ValueRepresentation::UR,
      "US" => ValueRepresentation::US,
      "UT" => ValueRepresentation::UT,
      "UV" => ValueRepresentation::UV,
      _ => ValueRepresentation::UN,
    }
  }
}

pub fn to_xml_dicom_attribute(instance: &Instance, dicom_attribute: &instance::DicomAttribute)
  -> Result<DicomAttribute, DicomError> {
  let dicom_value = DicomValue::from_dicom_attribute(dicom_attribute, instance)?;
  let payload = match dicom_value {
    DicomValue::OB(content) =>
      Some(Payload::Value(vec![ValuePayload::String(general_purpose::STANDARD.encode(content))])),
    DicomValue::OW(content) => {
      let content8: &[u8] = unsafe {
        std::slice::from_raw_parts(content.as_ptr() as *const u8, content.len() / 2)
      };
      Some(Payload::Value(vec![ValuePayload::String(general_purpose::STANDARD.encode(content8))]))
    },
    DicomValue::IS(value) => {
      if let Ok(value) = value.parse::<i64>() {
        Some(Payload::Value(vec![ValuePayload::Numeral(value as f64)]))
      } else {
        None
      }
    },
    DicomValue::SL(value) => Some(Payload::Value(vec![ValuePayload::Numeral(value.into())])),
    DicomValue::SS(value) => Some(Payload::Value(vec![ValuePayload::Numeral(value.into())])),
    DicomValue::UL(value) => Some(Payload::Value(vec![ValuePayload::Numeral(value.into())])),
    DicomValue::US(value) => Some(Payload::Value(vec![ValuePayload::Numeral(value.into())])),
    DicomValue::PN(value) => Some(Payload::Value(vec![ValuePayload::PersonName(PersonName::Alphabetic(NameVariant::Name(value)))])),
    DicomValue::SQ(_) |
    DicomValue::SeqItem(_) => {
      let dicom_attributes: Result<Vec<DicomAttribute>, DicomError> = dicom_attribute.subattributes.iter().map(|da| {
        to_xml_dicom_attribute(instance, da)
      }).collect();
      Some(Payload::Item(dicom_attributes?))
    }
    _ => Some(Payload::Value(vec![ValuePayload::String(dicom_value.to_string())])),
  };

  Ok(DicomAttribute {
    tag: format!("{:04x}{:04x}", dicom_attribute.tag.group, dicom_attribute.tag.element),
    vr: dicom_attribute.vr.as_ref().into(),
    keyword: Some(dicom_attribute.tag.name.to_string()),
    private_creator: None,
    payload: payload,
  })
}

// Convert a DICOM file to a the XML model. We need to specialize it to XML because
// of the difference between XML and JSON that the DICOM norm introduced.
pub fn dcm2native_dicom_model(f: File) -> Result<NativeDicomModel, Box<dyn Error>> {
  let instance = Instance::from_buf_reader(BufReader::new(f))?;
  let mut dicom_attributes = Vec::<DicomAttribute>::new();
  for dicom_attribute in instance.iter() {
    let dicom_attribute = dicom_attribute?;
    dicom_attributes.push(to_xml_dicom_attribute(&instance, &dicom_attribute)?);
  }
  Ok(NativeDicomModel { dicom_attributes: dicom_attributes })
}

pub fn to_json_dicom_attribute(instance: &Instance, dicom_attribute: &instance::DicomAttribute)
  -> Result<DicomAttributeJson, DicomError> {
  let dicom_value = DicomValue::from_dicom_attribute(dicom_attribute, instance)?;
  let payload = match dicom_value {
    DicomValue::OB(content) =>
      Some(Payload::Value(vec![ValuePayload::String(general_purpose::STANDARD.encode(content))])),
    DicomValue::OW(content) => {
      let content8: &[u8] = unsafe {
        std::slice::from_raw_parts(content.as_ptr() as *const u8, content.len() / 2)
      };
      Some(Payload::Value(vec![ValuePayload::String(general_purpose::STANDARD.encode(content8))]))
    },
    DicomValue::IS(value) => {
      if let Ok(value) = value.parse::<i64>() {
        Some(Payload::Value(vec![ValuePayload::Numeral(value as f64)]))
      } else {
        None
      }
    },
    DicomValue::SL(value) => Some(Payload::Value(vec![ValuePayload::Numeral(value.into())])),
    DicomValue::SS(value) => Some(Payload::Value(vec![ValuePayload::Numeral(value.into())])),
    DicomValue::UL(value) => Some(Payload::Value(vec![ValuePayload::Numeral(value.into())])),
    DicomValue::US(value) => Some(Payload::Value(vec![ValuePayload::Numeral(value.into())])),
    DicomValue::PN(value) => Some(Payload::Value(vec![ValuePayload::PersonName(PersonName::Alphabetic(NameVariant::Name(value)))])),
    DicomValue::SQ(_) => {
      let mut values = Vec::<ValuePayload>::new();
      for da in &dicom_attribute.subattributes {
        if let Some(Payload::Value(mut v)) = to_json_dicom_attribute(instance, da)?.payload.take() {
          values.push(v.swap_remove(0));
        }
      }
      Some(Payload::Value(values))
    },
    DicomValue::SeqItem(_) => {
      let mut dicom_attributes = BTreeMap::<String, DicomAttributeJson>::new();
      for da in &dicom_attribute.subattributes {
        let tag = format!("{:04x}{:04x}", da.tag.group, da.tag.element);
        dicom_attributes.insert(tag, to_json_dicom_attribute(instance, da)?);
      }
      Some(Payload::Value(vec![ValuePayload::Sequence(dicom_attributes)]))
    },
    _ => Some(Payload::Value(vec![ValuePayload::String(dicom_value.to_string())])),
  };

  Ok(DicomAttributeJson {
    vr: dicom_attribute.vr.as_ref().into(),
    keyword: Some(dicom_attribute.tag.name.to_string()),
    private_creator: None,
    payload: payload,
  })
}

// Convert a DICOM file to a the XML model. We need to specialize it to XML because
// of the difference between XML and JSON that the DICOM norm introduced.
pub fn dcm2json(f: File) -> Result<BTreeMap<String, DicomAttributeJson>, Box<dyn Error>> {
  let instance = Instance::from_buf_reader(BufReader::new(f))?;
  let mut dicom_attributes = BTreeMap::<String, DicomAttributeJson>::new();
  for dicom_attribute in instance.iter() {
    let dicom_attribute = dicom_attribute?;
    // println!("{:?}", dicom_attribute);
    let tag = format!("{:04x}{:04x}", dicom_attribute.tag.group, dicom_attribute.tag.element);
    dicom_attributes.insert(tag, to_json_dicom_attribute(&instance, &dicom_attribute)?);
  }
  Ok(dicom_attributes)
}

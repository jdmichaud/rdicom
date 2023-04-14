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
// TODO: Remove that external dependency if possible
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
            ValuePayload::Numeral(n) => n.to_string(),
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

impl TryFrom<Payload> for Vec<f32> {
  type Error = DicomError;

  fn try_from(payload: Payload) -> Result<Self, Self::Error> {
    match payload {
      Payload::Value(value) => value.iter()
        .map(|v| match v {
          ValuePayload::Numeral(f32_value) => Ok(*f32_value as f32),
          _ => Err(DicomError::new("Payload is not a f32")),
        }).collect(),
      _ => Err(DicomError::new("Payload is not a f32")),
    }
  }
}

impl TryFrom<Payload> for Vec<f64> {
  type Error = DicomError;

  fn try_from(payload: Payload) -> Result<Self, Self::Error> {
    match payload {
      Payload::Value(value) => value.iter()
        .map(|v| match v {
          ValuePayload::Numeral(f64_value) => Ok(*f64_value as f64),
          _ => Err(DicomError::new("Payload is not a f64")),
        }).collect(),
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
      Some(Payload::Value(value.iter()
        .filter_map(|v| v.parse::<i64>().ok())
        .map(|v| ValuePayload::Numeral(v as f64))
        .collect::<Vec<ValuePayload>>()))
    },
    DicomValue::SL(value) => Some(Payload::Value(vec![ValuePayload::Numeral(value.into())])),
    DicomValue::SS(value) => Some(Payload::Value(vec![ValuePayload::Numeral(value.into())])),
    DicomValue::UL(value) => Some(Payload::Value(vec![ValuePayload::Numeral(value.into())])),
    DicomValue::US(value) => Some(Payload::Value(vec![ValuePayload::Numeral(value.into())])),
    DicomValue::PN(value) => Some(Payload::Value(vec![ValuePayload::PersonName(PersonName::Alphabetic(NameVariant::Name(value[0].clone())))])),
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
      Some(Payload::Value(value.iter()
        .filter_map(|v| v.parse::<i64>().ok())
        .map(|v| ValuePayload::Numeral(v as f64))
        .collect::<Vec<ValuePayload>>()))
    },
    DicomValue::SL(value) => Some(Payload::Value(vec![ValuePayload::Numeral(value.into())])),
    DicomValue::SS(value) => Some(Payload::Value(vec![ValuePayload::Numeral(value.into())])),
    DicomValue::UL(value) => Some(Payload::Value(vec![ValuePayload::Numeral(value.into())])),
    DicomValue::US(value) => Some(Payload::Value(vec![ValuePayload::Numeral(value.into())])),
    DicomValue::PN(value) => Some(Payload::Value(vec![ValuePayload::PersonName(PersonName::Alphabetic(NameVariant::Name(value[0].clone())))])),
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

pub mod json2dcm {

use std::io::Write;
use crate::dicom_representation::ValuePayload;
use crate::dicom_representation::Payload;
use crate::dicom_representation::DicomAttributeJson;
use crate::dicom_representation::BTreeMap;
use crate::dicom_representation::ValueRepresentation;
use crate::dicom_representation::DicomAttribute;
use std::error::Error;
use std::io::BufWriter;

fn write_even_16<W: std::io::Write>(writer: &mut BufWriter<W>, data: &[u8], padchar: u8) -> Result<usize, Box<dyn Error>> {
  let data_length = data.len();
  // length must be even so if odd, pad with 0
  let pad_length = if data_length % 2 == 0 { 0 } else { 1 };
  writer.write(&((data_length + pad_length) as u16).to_le_bytes())?;
  writer.write(data)?;
  if pad_length == 1 {
    writer.write(&[padchar])?;
  }
  Ok(data_length + pad_length)
}

fn write_even_32<W: std::io::Write>(writer: &mut BufWriter<W>, data: &[u8]) -> Result<usize, Box<dyn Error>> {
  let data_length = data.len();
  // length must be even so if odd, pad with 0
  let pad_length = if data_length % 2 == 0 { 0 } else { 1 };
  writer.write(&[0, 0])?;
  writer.write(&((data_length + pad_length) as u32).to_le_bytes())?;
  writer.write(data)?;
  if pad_length == 1 {
    writer.write(&[0])?;
  }
  Ok(data_length + pad_length)
}

fn serialize<W: std::io::Write>(writer: &mut BufWriter<W>, dicom_attribute: DicomAttribute) -> Result<usize, Box<dyn Error>> {
  let group_h: u8 = u8::from_str_radix(&dicom_attribute.tag[0..2], 16)?;
  let group_l: u8 = u8::from_str_radix(&dicom_attribute.tag[2..4], 16)?;
  let element_h: u8 = u8::from_str_radix(&dicom_attribute.tag[4..6], 16)?;
  let element_l: u8 = u8::from_str_radix(&dicom_attribute.tag[6..8], 16)?;
  // TODO: Isn't there a better looking way?
  let vr: &[u8] = <ValueRepresentation as Into<&str>>::into(dicom_attribute.vr).as_bytes();
  let mut length: usize = 6;
  writer.write(&[group_l, group_h, element_l, element_h, vr[0], vr[1]])?;

  // https://dicom.nema.org/dicom/2013/output/chtml/part05/chapter_7.html#sect_7.1.2
  if let Some(payload) = dicom_attribute.payload {
    match dicom_attribute.vr {
      // The following VRs expect 2 bytes of padding ([0, 0]) and a 4 bytes length
      ValueRepresentation::UN |
      ValueRepresentation::OW |
      ValueRepresentation::OB => {
        let data: Vec<u8> = payload.try_into()?;
        length += 6 + write_even_32(writer, &data.as_slice())?;
      },
      ValueRepresentation::SQ => {
        let mut items_buffer: Vec<u8> = Vec::<u8>::new();
        if let Payload::Value(items) = payload {
          // Stream all the items as Explicit length to an array
          // Write the size of the array on 4 bytes
          // Write the array

          let mut items_buffer_writer = BufWriter::new(&mut items_buffer);
          for item in items {
            let mut subfields_buffer: Vec<u8> = Vec::<u8>::new();
            let mut subfields_written: u32 = 0;
            if let ValuePayload::Sequence(subfields) = item {
              let mut subfields_buffer_writer = BufWriter::new(&mut subfields_buffer);
              for (tag, attribute) in subfields.iter() {
                subfields_written += serialize(&mut subfields_buffer_writer, DicomAttribute {
                  tag: tag.to_string(),
                  vr: attribute.vr,
                  payload: attribute.payload.clone(), // TODO: get rid of clone here
                  keyword: None, private_creator: None,
                })? as u32;
              }
            }
            // item tag
            items_buffer_writer.write(&[0xFE, 0xFF, 0x00, 0xE0])?;
            // item length on 4 bytes
            items_buffer_writer.write(&subfields_written.to_le_bytes())?;
            // subfileds data
            items_buffer_writer.write(&subfields_buffer.as_slice())?;
          }
        }
        length += 6 + write_even_32(writer, &items_buffer.as_slice())?;
      },
      ValueRepresentation::UT => {
        let data: String = payload.try_into()?;
        length += 6 + write_even_32(writer, &data.as_bytes())?;
      },
      ValueRepresentation::OF => { unimplemented!() },
      // The following VRs expect a 2 bytes length
      ValueRepresentation::AT |
      ValueRepresentation::SL |
      ValueRepresentation::UC => { todo!("{:?}", dicom_attribute.vr); },
      ValueRepresentation::AE |
      ValueRepresentation::AS |
      ValueRepresentation::CS |
      ValueRepresentation::DA |
      ValueRepresentation::DS |
      ValueRepresentation::DT |
      ValueRepresentation::LO |
      ValueRepresentation::LT |
      ValueRepresentation::SH |
      ValueRepresentation::PN |
      ValueRepresentation::ST |
      ValueRepresentation::TM => {
        // Strings are padded with space (0x20)
        // https://dicom.nema.org/dicom/2013/output/chtml/part05/sect_6.2.html
        let data: String = payload.try_into()?;
        length += 2 + write_even_16(writer, &data.as_bytes(), 0x20)?;
      },
      ValueRepresentation::UI => {
        // UI is padded with 0
        // https://dicom.nema.org/dicom/2013/output/chtml/part05/sect_6.2.html
        let data: String = payload.try_into()?;
        length += 2 + write_even_16(writer, &data.as_bytes(), 0x0)?;
      },
      ValueRepresentation::IS => {
        let as_is: String = payload.try_into()?;
        let data: String = as_is.split(".").take(1).collect::<_>();
        length += 2 + write_even_16(writer, &data.as_bytes(), 0x20)?;
      }
      ValueRepresentation::UL => {
        let value: u32 = payload.try_into()?;
        writer.write(&(std::mem::size_of_val(&value) as u16).to_le_bytes())?;
        writer.write(&(value.to_le_bytes()))?;
        length += 2 + std::mem::size_of_val(&value);
      },
      ValueRepresentation::SS => {
        let value: i16 = payload.try_into()?;
        writer.write(&(std::mem::size_of_val(&value) as u16).to_le_bytes())?;
        writer.write(&value.to_le_bytes())?;
        length += 2 + std::mem::size_of_val(&value);
      },
      ValueRepresentation::US => {
        let value: u16 = payload.try_into()?;
        writer.write(&(std::mem::size_of_val(&value) as u16).to_le_bytes())?;
        writer.write(&value.to_le_bytes())?;
        length += 2 + std::mem::size_of_val(&value);
      },
      ValueRepresentation::FL => {
        let value: Vec<f32> = payload.try_into()?;
        let data_length = std::mem::size_of_val(&value[0]) * value.len();
        writer.write(&(data_length as u16).to_le_bytes())?;
        for v in value {
          writer.write(&v.to_le_bytes())?;
        }
        length += 2 + data_length;
      },
      ValueRepresentation::FD => {
        let value: Vec<f64> = payload.try_into()?;
        let data_length = std::mem::size_of_val(&value[0]) * value.len();
        writer.write(&(data_length as u16).to_le_bytes())?;
        for v in value {
          writer.write(&v.to_le_bytes())?;
        }
        length += 2 + data_length;
      },
      // TODO: No DicomValue equivalent for now
      ValueRepresentation::UR |
      ValueRepresentation::UV |
      ValueRepresentation::OD |
      ValueRepresentation::OL |
      ValueRepresentation::OV |
      ValueRepresentation::SV => { unimplemented!(); } // No DicomValue variant in instance.rs
    }
  } else {
    match dicom_attribute.vr {
      ValueRepresentation::OB |
      ValueRepresentation::OW |
      ValueRepresentation::OF |
      ValueRepresentation::SQ |
      ValueRepresentation::UT |
      ValueRepresentation::UN => {
        writer.write(&[0, 0, 0, 0, 0, 0])?;
        length += 6;
      },
      _ => {
        writer.write(&[0, 0])?;
        length += 2;
      }
    }
  }
  Ok(length)
}

pub fn json2dcm<W: std::io::Write>(writer: &mut BufWriter<W>, json: &BTreeMap<String, DicomAttributeJson>) -> Result<(), Box<dyn Error>> {
  // Write the DICOM header
  writer.write(&[0; 0x80])?;
  writer.write(&[b'D', b'I', b'C', b'M'])?;
  // Write the meta-information header
  let mut meta_info_header: Vec<u8> = Vec::<u8>::new();
  {
    let mut meta_info_header_writer = BufWriter::new(&mut meta_info_header);
  // (0002,0002) UI =SecondaryCaptureImageStorage            #  26, 1 MediaStorageSOPClassUID
    let sop_class_uid = String::try_from(
      &json.get("00080016").ok_or("Missing SOPClassUID")?.payload.clone().ok_or("Missing SOPClassUID")?
    )?;
    let mut written = serialize(&mut meta_info_header_writer, DicomAttribute {
      tag: "00020002".to_string(),
      vr: ValueRepresentation::UI,
      payload: Some(Payload::Value(vec![ValuePayload::String(sop_class_uid)])),
      keyword: None, private_creator: None,
    })?;
  // (0002,0003) UI [1.2.826.0.1.3680043.8.1055.1.20111103112244831.30826609.78057758] #  64, 1 MediaStorageSOPInstanceUID
    let sop_instance_uid = String::try_from(
      &json.get("00080018").ok_or("Missing SOPInstanceUID")?.payload.clone().ok_or("Missing SOPInstanceUID")?
    )?;
    written += serialize(&mut meta_info_header_writer, DicomAttribute {
      tag: "00020003".to_string(),
      vr: ValueRepresentation::UI,
      payload: Some(Payload::Value(vec![ValuePayload::String(sop_instance_uid)])),
      keyword: None, private_creator: None,
    })?;
  // (0002,0010) UI =LittleEndianImplicit                    #  18, 1 TransferSyntaxUID
    written += serialize(&mut meta_info_header_writer, DicomAttribute {
      tag: "00020010".to_string(),
      vr: ValueRepresentation::UI,
      payload: Some(Payload::Value(vec![ValuePayload::String("1.2.840.10008.1.2.1".to_string())])),
      keyword: None, private_creator: None,
    })?;
  // (0002,0012) UI [1.2.826.0.1.3680043.8.1055.1]           #  28, 1 ImplementationClassUID
    written += serialize(&mut meta_info_header_writer, DicomAttribute {
      tag: "00020012".to_string(),
      vr: ValueRepresentation::UI,
      payload: Some(Payload::Value(vec![ValuePayload::String("1.2.826.0.1.3680043.8.1055.1".to_string())])),
      keyword: None, private_creator: None,
    })?;
    meta_info_header_writer.flush()?;
    // Write FileMetaInformationGroupLength
    serialize(writer, DicomAttribute {
      tag: "00020000".to_string(),
      vr: ValueRepresentation::UL,
      payload: Some(Payload::Value(vec![ValuePayload::Numeral(written as f64)])),
      keyword: None, private_creator: None,
    })?;
  }
  // Write the other meta information
  writer.write(&meta_info_header.as_slice())?;
  // Write the rest of the dicom attributes
  for (tag, attribute) in json.iter() {
    serialize(writer, DicomAttribute {
      tag: tag.to_string(),
      vr: attribute.vr,
      payload: attribute.payload.clone(), // TODO: get rid of clone here
      keyword: None, private_creator: None,
    })?;
  }
  writer.flush()?;
  Ok(())
}
}

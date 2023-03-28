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

use serde::{Deserialize, Serialize};

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

#[derive(Debug, Serialize, Deserialize)]
pub struct NativeDicomModel {
  #[serde(rename = "DicomAttribute")]
  pub dicom_attributes: Vec<DicomAttribute>,
}

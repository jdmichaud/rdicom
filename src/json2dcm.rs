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

use std::collections::BTreeMap;
use std::io::Write;
use std::io::BufWriter;
use rdicom::dicom_representation::{DicomAttributeJson, DicomAttribute, ValueRepresentation, Payload, ValuePayload};
use std::io::BufReader;
use std::error::Error;
use std::fs::File;
use structopt::StructOpt;
use structopt::clap::AppSettings;

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
          let mut items_written: u32 = 0;
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
            items_written += 4;
            // item length on 4 bytes
            items_buffer_writer.write(&subfields_written.to_le_bytes())?;
            items_written += 4;
            // subfileds data
            items_buffer_writer.write(&subfields_buffer.as_slice())?;
            items_written += subfields_written;
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
        length += 2 + &std::mem::size_of_val(&value);
      },
      ValueRepresentation::SS => {
        let value: i16 = payload.try_into()?;
        writer.write(&(std::mem::size_of_val(&value) as u16).to_le_bytes())?;
        writer.write(&value.to_le_bytes())?;
        length += 2 + &std::mem::size_of_val(&value);
      },
      ValueRepresentation::US => {
        let value: u16 = payload.try_into()?;
        writer.write(&(std::mem::size_of_val(&value) as u16).to_le_bytes())?;
        writer.write(&value.to_le_bytes())?;
        length += 2 + &std::mem::size_of_val(&value);
      },
      ValueRepresentation::FL => {
        let value: f32 = payload.try_into()?;
        writer.write(&(std::mem::size_of_val(&value) as u16).to_le_bytes())?;
        writer.write(&value.to_le_bytes())?;
        length += 2 + &std::mem::size_of_val(&value);
      },
      ValueRepresentation::FD => {
        let value: f64 = payload.try_into()?;
        writer.write(&(std::mem::size_of_val(&value) as u16).to_le_bytes())?;
        writer.write(&value.to_le_bytes())?;
        length += 2 + &std::mem::size_of_val(&value);
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

fn main() -> Result<(), Box<dyn Error>> {
  let opt = Opt::from_args();
  let inputfile = File::open(&opt.jsonfilepath)?;
  let mut result: BTreeMap<String, DicomAttributeJson> = serde_json::from_reader(BufReader::new(inputfile))?;

  let outputfile = File::create(&opt.dcmfilepath)?;
  let mut writer = BufWriter::new(outputfile);
  // Write the DICOM header
  writer.write(&[0; 0x80])?;
  writer.write(&[b'D', b'I', b'C', b'M'])?;
  // Write the meta-information header
  let mut meta_info_header: Vec<u8> = Vec::<u8>::new();
  {
    let mut meta_info_header_writer = BufWriter::new(&mut meta_info_header);
  // (0002,0002) UI =SecondaryCaptureImageStorage            #  26, 1 MediaStorageSOPClassUID
    let sop_class_uid = String::try_from(
      &result.get_mut("00080016").ok_or("Missing SOPClassUID")?.payload.clone().ok_or("Missing SOPClassUID")?
    )?;
    let mut written = serialize(&mut meta_info_header_writer, DicomAttribute {
      tag: "00020002".to_string(),
      vr: ValueRepresentation::UI,
      payload: Some(Payload::Value(vec![ValuePayload::String(sop_class_uid)])),
      keyword: None, private_creator: None,
    })?;
  // (0002,0003) UI [1.2.826.0.1.3680043.8.1055.1.20111103112244831.30826609.78057758] #  64, 1 MediaStorageSOPInstanceUID
    let sop_instance_uid = String::try_from(
      &result.get_mut("00080018").ok_or("Missing SOPInstanceUID")?.payload.clone().ok_or("Missing SOPInstanceUID")?
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
    serialize(&mut writer, DicomAttribute {
      tag: "00020000".to_string(),
      vr: ValueRepresentation::UL,
      payload: Some(Payload::Value(vec![ValuePayload::Numeral(written as f64)])),
      keyword: None, private_creator: None,
    })?;
  }
  // Write the other meta information
  writer.write(&meta_info_header.as_slice())?;
  // Write the rest of the dicom attributes
  for (tag, attribute) in result.iter() {
    serialize(&mut writer, DicomAttribute {
      tag: tag.to_string(),
      vr: attribute.vr,
      payload: attribute.payload.clone(), // TODO: get rid of clone here
      keyword: None, private_creator: None,
    })?;
  }
  writer.flush()?;
  Ok(())
}

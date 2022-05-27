#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]

// https://radu-matei.com/blog/practical-guide-to-wasm-memory/#passing-arrays-to-rust-webassembly-modules

use std::convert::TryInto;
use std::borrow::Cow;
use std::error::Error;
use std::io::Read;
use std::io::BufReader;
use std::str::from_utf8;
use std::fmt;

use crate::misc::has_dicom_header;
use crate::error::DicomError;
use crate::tags::Tag;
use crate::dicom_tags::Item;
use crate::dicom_tags::ItemDelimitationItem;
use crate::dicom_tags::SequenceDelimitationItem;

#[derive(Debug)]
pub struct Instance {
  pub buffer: Vec<u8>,
}

#[derive(Debug)]
pub enum DicomValue<'a> {
  AE(String),
  AS(String),
  AT(Tag),
  CS(String),
  DA(String),
  DS(String),
  DT(String),
  FD(f64),
  FL(f32),
  IS(String),
  LO(String),
  LT(String),
  OB(&'a [u8]),
  OW(&'a [u16]),
  PN(String),
  SeqEnd,
  SeqItem,
  SeqItemEnd,
  SH(String),
  SL(i32),
  SQ(Vec<DicomAttribute<'a>>),
  SS(i16),
  ST(String),
  TM(String),
  UI(String),
  UL(u32),
  US(u16),
  UT(String),
}

impl<'a> ToString for DicomValue<'a> {
  fn to_string(&self) -> String {
    match self {
      DicomValue::AE(value) |
      DicomValue::AS(value) |
      DicomValue::CS(value) |
      DicomValue::DA(value) |
      DicomValue::IS(value) |
      DicomValue::LO(value) |
      DicomValue::LT(value) |
      DicomValue::SH(value) |
      DicomValue::ST(value) |
      DicomValue::TM(value) |
      DicomValue::UT(value) => format!("{}", value),
      DicomValue::DS(value) => format!("{}", value),
      DicomValue::DT(value) => format!("{}", value),
      DicomValue::FD(value) => format!("{}", value),
      DicomValue::FL(value) => format!("{}", value),
      DicomValue::OB(value) => {
        let mut result = String::with_capacity(40);
        let mut it = (*value).into_iter().peekable();
        while let Some(n) = it.next()  {
            result.push_str(&format!("{:02x}", n));
            if result.len() >= 64 {
              result.replace_range(64.., "...");
              break;
            }
            if !it.peek().is_none() {
              result.push_str("\\");
            }
        }
        result
      },
      DicomValue::OW(value) => {
        let mut result = String::with_capacity(40);
        let mut it = (*value).into_iter().peekable();
        while let Some(n) = it.next()  {
            result.push_str(&format!("{:04x}", n));
            if result.len() >= 64 {
              result.replace_range(64.., "...");
              break;
            }
            if !it.peek().is_none() {
              result.push_str("\\");
            }
        }
        result
      },
      DicomValue::PN(value) => format!("{}", value),
      // DicomValue::SeqEnd,
      // DicomValue::SeqItem,
      // DicomValue::SeqItemEnd,
      DicomValue::SL(value) => format!("{}", value),
      // DicomValue::SQ(value),
      DicomValue::SS(value) => format!("{}", value),
      DicomValue::UI(value) => format!("[{}]", value),
      DicomValue::UL(value) => format!("{}", value),
      DicomValue::US(value) => format!("{}", value),
      _ => unimplemented!("No formatter for {:?}", self),
    }
  }
}

impl<'a> DicomValue<'a> {
  pub fn from_dicom_field<'b>(dicom_field: &DicomAttribute<'b>, instance: &'b Instance) -> DicomValue<'b> {
    match (dicom_field.group, dicom_field.element) {
      (0xFFFE, 0xE000) => DicomValue::SeqItem,
      (0xFFFE, 0xE00D) => DicomValue::SeqItemEnd,
      (0xFFFE, 0xE0DD) => DicomValue::SeqEnd,
      _ => DicomValue::new(&dicom_field.vr, dicom_field.offset, dicom_field.length, &instance.buffer),
    }
  }

  fn new_sequence<'b>(fields: Vec<DicomAttribute<'b>>) -> DicomValue<'b> {
    DicomValue::SQ(fields)
  }

  fn new<'b>(vr: &str, offset: usize, length: usize, buffer: &'b Vec<u8>) -> DicomValue<'b> {
    match vr {
      "AE" => DicomValue::AE(
        from_utf8(&buffer[offset..offset + length])
          .unwrap()
          .trim_matches(char::from(0))
          .trim()
          .to_string()
      ),
      "AS" => DicomValue::AS(
        from_utf8(&buffer[offset..offset + length])
          .unwrap()
          .trim_matches(char::from(0))
          .trim()
          .to_string()
      ),
      "AT" => {
        let tmp: [u8; 4] = buffer[offset..offset + 4].try_into().unwrap();
        DicomValue::AT(u32::from_le_bytes(tmp).into())
      },
      "CS" => DicomValue::CS(
        from_utf8(&buffer[offset..offset + length])
          .unwrap()
          .trim_matches(char::from(0))
          .trim()
          .to_string()
      ),
      "DA" => DicomValue::DA(
        from_utf8(&buffer[offset..offset + length])
          .unwrap()
          .trim_matches(char::from(0))
          .trim()
          .to_string()
      ),
      "DS" => DicomValue::DS(
        from_utf8(&buffer[offset..offset + length])
          .unwrap()
          .trim_matches(char::from(0))
          .trim()
          .to_string()
      ),
      "DT" => DicomValue::DT(
        from_utf8(&buffer[offset..offset + length])
          .unwrap()
          .trim_matches(char::from(0))
          .trim()
          .to_string()
      ),
      "FD" => {
        let tmp: [u8; 8] = buffer[offset..offset + 8].try_into().unwrap();
        DicomValue::FD(f64::from_le_bytes(tmp))
      }
      "FL" => {
        let tmp: [u8; 4] = buffer[offset..offset + 4].try_into().unwrap();
        DicomValue::FL(f32::from_le_bytes(tmp))
      }
      "IS" => DicomValue::IS(
        from_utf8(&buffer[offset..offset + length])
          .unwrap()
          .trim_matches(char::from(0))
          .trim()
          .to_string()
      ),
      "LO" => DicomValue::LO(
        from_utf8(&buffer[offset..offset + length])
          .unwrap()
          .trim_matches(char::from(0))
          .trim()
          .to_string()
      ),
      "LT" => DicomValue::LT(
        from_utf8(&buffer[offset..offset + length])
          .unwrap()
          .trim_matches(char::from(0))
          .trim()
          .to_string()
      ),
      "OB" => DicomValue::OB(&buffer[offset..offset + length]),
      "OW" => {
        let (_, owslice, _) = unsafe {
          buffer[offset..offset + length].align_to::<u16>()
        };
        return DicomValue::OW(owslice);
      },
      "PN" => DicomValue::PN(
        from_utf8(&buffer[offset..offset + length])
          .unwrap()
          .trim_matches(char::from(0))
          .trim()
          .to_string()
      ),
      "SH" => DicomValue::SH(
        from_utf8(&buffer[offset..offset + length])
          .unwrap()
          .trim_matches(char::from(0))
          .trim()
          .to_string()
      ),
      "SL" => DicomValue::SL(
        buffer[offset] as i32 |
        (buffer[offset + 1] as i32) << 8 |
        (buffer[offset + 2] as i32) << 16 |
        (buffer[offset + 3] as i32) << 24
      ),
      "SS" => DicomValue::SS(
        buffer[offset] as i16 |
        (buffer[offset + 1] as i16) << 8
      ),
      "ST" => DicomValue::ST(
        from_utf8(&buffer[offset..offset + length])
          .unwrap()
          .trim_matches(char::from(0))
          .trim()
          .to_string()
      ),
      "TM" => DicomValue::TM(
        from_utf8(&buffer[offset..offset + length])
          .unwrap()
          .trim_matches(char::from(0))
          .trim()
          .to_string()
      ),
      "UI" => DicomValue::UI(
        from_utf8(&buffer[offset..offset + length])
          .unwrap()
          .trim_matches(char::from(0))
          .trim()
          .to_string()
      ),
      "UL" => DicomValue::UL(
        buffer[offset] as u32 |
        (buffer[offset + 1] as u32) << 8 |
        (buffer[offset + 2] as u32) << 16 |
        (buffer[offset + 3] as u32) << 24
      ),
      "US" => DicomValue::US(
        buffer[offset] as u16 |
        (buffer[offset + 1] as u16) << 8
      ),
      "UT" => DicomValue::UT(
        from_utf8(&buffer[offset..offset + length])
          .unwrap()
          .trim_matches(char::from(0))
          .trim()
          .to_string()
      ),
      _ => unimplemented!("Value representation \"{}\" not implemented", vr),
    }
  }
}

#[derive(Debug)]
pub struct DicomAttribute<'a> {
  pub group: u16,
  pub element: u16,
  pub vr: Cow<'a, str>,
  pub length: usize,
  // Position of the tag content in the buffer after the VR and length
  pub offset: usize,
  pub tag: Tag,
}

impl<'a> DicomAttribute<'a> {
  pub fn new<S>(group: u16, element: u16, vr: S, offset: usize, length: usize,
    tag: Tag) -> DicomAttribute<'a>
    where S: Into<Cow<'a, str>> {
    DicomAttribute { group, element, vr: vr.into(), length, offset, tag }
  }
}

impl Instance {
  pub fn from_buf_reader<T: Read>(mut buf_reader: BufReader<T>) -> Result<Self, Box<dyn Error>> {
    // Read the whole file into a buffer
    let mut buffer: Vec<u8> = vec![];
    buf_reader.read_to_end(&mut buffer)?;
    Instance::from(buffer)
  }

  pub fn from(buffer: Vec<u8>) -> Result<Self, Box<dyn Error>> {
    // Check it's a DICOM file
    if !has_dicom_header(&buffer) {
      return Err(Box::new(DicomError::new("Not a DICOM file")));
    }

    let instance = Instance { buffer };

    if let Err(e) = instance.is_supported_type() {
      Err(e)
    } else {
      Ok(instance)
    }
  }

  // #[no_mangle]
  // pub extern "C" fn from_ptr(ptr: *mut u8, len: usize) -> Self {
  //   let v = unsafe { Vec::from_raw_parts(ptr, len, len) };
  //   match Self::from(v) {
  //     Ok(instance) => instance,
  //     Err(_) => panic!("Find a way to raise a Javascript exception here"),
  //   }
  // }

  pub fn get_value<'a>(self: &'a Self, tag: &Tag) -> Option<DicomValue> {
    // Fast forward the DICOM prefix
    // TODO: Deal with non-comformant DICOM files
    println!("get_tab {:?}", tag);
    let mut offset = 128 + "DICM".len();
    return loop {
      println!("{:#06x} {:#06x}", offset, self.buffer.len());
      let field = self.next_attribute(offset).unwrap();
      offset = field.offset;

      if field.group == tag.group && field.element == tag.element {
        let value = if field.vr == "SQ" {
          let sequence_length = field.length;
          // See http://dicom.nema.org/dicom/2013/output/chtml/part05/sect_7.5.html
          // on what a sequence look like in a DICOM file.
          let mut subfields: Vec<DicomAttribute<'a>> = vec![];
          let start = offset;
          // Advance until we find a SeqEnd
          loop {
            let subfield = self.next_attribute(offset).unwrap();
            let value = DicomValue::from_dicom_field(&subfield, &self);
            match value {
              DicomValue::SeqEnd => {
                // We found the end of the sequence, create a DicomAttribute with a Sequence container
                break DicomValue::new_sequence(subfields);
              }
              _ => {
                // Moving along the tags inside the sequence
                let subfield_length = subfield.length;
                let subfield_offset = subfield.offset;
                offset = subfield_offset + subfield_length;
                subfields.push(subfield);
                if offset - start >= sequence_length {
                  break DicomValue::new_sequence(subfields);
                }
              }
            }
          }
        } else {
          DicomValue::from_dicom_field(&field, &self)
        };
        break Some(value);
      }
      println!("field.offset {} field.length {}", field.offset, field.length);
      offset = field.offset + if field.length == 0xFFFFFFF { 0 } else { field.length };
      if offset >= self.buffer.len() {
        println!("stop here {:#06x} {:#06x}", offset, self.buffer.len());
        break None
      }
    };
  }

  pub fn next_attribute<'a>(self: &'a Self, offset: usize) -> Option<DicomAttribute<'a>> {
    // group(u16),element(u16),vr(str[2]),length(u16)
    let mut offset = offset;
    if offset >= self.buffer.len() {
      return None
    }
    let group = self.buffer[offset] as u16 | (self.buffer[offset + 1] as u16) << 8;
    let element = self.buffer[offset + 2] as u16 | (self.buffer[offset + 3] as u16) << 8;
    println!("{:#06x?}:{:#06x?}", group, element);
    offset += 4; // Skip group and element
    // Check if we have sequence related data element
    if group == 0xFFFE {
      // Sequence delimiter items do not have VR and length and are always followed by 0xFFFFFFFF
      let length = self.buffer[offset] as usize | (self.buffer[offset + 1] as usize) << 8;
      offset += 4;
      return match element {
        0xE000 => Some(DicomAttribute::new(group, element, "", offset, length, Item)),
        0xE00D => Some(DicomAttribute::new(group, element, "", offset, length, ItemDelimitationItem)),
        0xE0DD => Some(DicomAttribute::new(group, element, "", offset, length, SequenceDelimitationItem)),
        _ => unimplemented!("unknown sequence related data element: {}", element),
      }
    }
    let vr = from_utf8(&self.buffer[offset..offset + 2])
      .expect(&format!("Error while reading value representation at {:#06x} while the buffer length is {}", offset, self.buffer.len()));
    offset += 2; // Skip VR
    let length: usize;
    if ["OB", "OD", "OF", "OL", "OW", "SQ", "UC", "UR", "UT", "UN"].contains(&vr) {
      // These VR types handles themselves differently. They have 2 reserved bytes
      // that need to be skipped and their data length is on 4 bytes.
      offset += 2; // Skip reserved byte
      if vr == "SQ" { // Sequence are special types within those special types... yikes.
        let tmp: [u8; 4] = self.buffer[offset..offset + 4].try_into().unwrap();
        length = u32::from_le_bytes(tmp) as usize; // Can sometimes be equal to 0xFFFFFFFF
        offset += 4;
      } else {
        length = self.buffer[offset] as usize |
          (self.buffer[offset + 1] as usize) << 8 |
          (self.buffer[offset + 2] as usize) << 16 |
          (self.buffer[offset + 3] as usize) << 24;
        offset += 4; // Skip length
      }
    } else {
      length = self.buffer[offset] as usize | (self.buffer[offset + 1] as usize) << 8;
      offset = offset + 2;
    }
    Some(DicomAttribute::new(group, element, vr, offset, length,
      (((group as u32) << 16) | element as u32).into()))
  }

  fn is_supported_type(self: &Self) -> Result<(), Box<dyn Error>> {
    // Only supporting little-endian explicit VR for now.
    if let Some(transfer_syntax_uid_field) = self.get_value(&0x00020010.into()) {
      match transfer_syntax_uid_field {
        DicomValue::UI(transfer_syntax_uid) => {
          if vec!["1.2.840.10008.1.2.1", "1.2.840.10008.1.2.4.70"].contains(&&*transfer_syntax_uid) {
            Ok(())
          }
          else {
            Err(Box::new(DicomError::new(
              &format!("Unsupported Transfer Syntax UID: {}", transfer_syntax_uid))))
          }
        }
        _ => Err(Box::new(DicomError::new(&format!("Unexpected type"))))
      }
    } else {
      Err(Box::new(DicomError::new("Transfer Syntax UID not found")))
    }
  }
}

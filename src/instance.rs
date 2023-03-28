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

// https://radu-matei.com/blog/practical-guide-to-wasm-memory/#passing-arrays-to-rust-webassembly-modules

use core::str::Utf8Error;
use std::fs::File;
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
  pub implicit: bool,
}

#[derive(Debug, PartialEq)]
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
  SeqItem(Vec<DicomValue<'a>>),
  SeqItemEnd,
  SH(String),
  SL(i32),
  SQ(Vec<DicomValue<'a>>),
  SS(i16),
  ST(String),
  TM(String),
  UI(String),
  UL(u32),
  US(u16),
  UT(String),
  UN(&'a [u8]),
}

// Convert Utf8Error to DicomError with a nice error message.
fn utf8_error_to_dicom_error(err: Utf8Error, tag: &str, offset: usize) -> DicomError {
  match err.error_len() {
    Some(l) => DicomError::new(&format!("UTF8 error: an unexpected byte was encountered while \
      decoding an {} tag at {:#x} + {}", tag, offset, l)),
    None => DicomError::new(&format!("UTF8 error: the end of the input was reached unexpectedly \
      while decoding an {} tag at {:#x}", tag, offset)),
  }
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
      DicomValue::UN(value) |
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
      DicomValue::UI(value) => format!("{}", value),
      DicomValue::UL(value) => format!("{}", value),
      DicomValue::US(value) => format!("{}", value),
      _ => unimplemented!("No formatter for {:?}", self),
    }
  }
}

impl<'a> DicomValue<'a> {
  pub fn from_dicom_attribute<'b>(attribute: &DicomAttribute<'b>, instance: &'b Instance)
    -> Result<DicomValue<'b>, DicomError> {
    Ok(match attribute.vr.as_ref() {
      "SQ" => {
        let values: Result<Vec<_>, _> = attribute.subattributes.iter()
          .map(|attribute| DicomValue::from_dicom_attribute(&attribute, instance))
          .collect::<_>();
        DicomValue::new_sequence(values?)
      },
      _ => match (attribute.group, attribute.element) {
        (0xFFFE, 0xE000) => {
          let values: Result<Vec<_>, _> = attribute.subattributes.iter()
            .map(|attribute| DicomValue::from_dicom_attribute(&attribute, instance))
            .collect::<_>();
          DicomValue::SeqItem(values?)
        },
        (0xFFFE, 0xE00D) => DicomValue::SeqItemEnd,
        (0xFFFE, 0xE0DD) => DicomValue::SeqEnd,
        _ => DicomValue::new(&attribute.vr, attribute.data_offset, attribute.data_length, &instance.buffer)?,
      }
    })
  }

  fn new_sequence<'b>(values: Vec<DicomValue<'b>>) -> DicomValue<'b> {
    DicomValue::SQ(values)
  }

  fn new_sequence_item<'b>(values: Vec<DicomValue<'b>>) -> DicomValue<'b> {
    DicomValue::SeqItem(values)
  }

  fn new<'b>(vr: &str, offset: usize, length: usize, buffer: &'b Vec<u8>) -> Result<DicomValue<'b>, DicomError> {
    Ok(match vr {
      "AE" => DicomValue::AE(
        from_utf8(&buffer[offset..offset + length])
          .map_err(|err| utf8_error_to_dicom_error(err, vr, offset))?
          .trim_matches(char::from(0))
          .trim()
          .to_string()
      ),
      "AS" => DicomValue::AS(
        from_utf8(&buffer[offset..offset + length])
          .map_err(|err| utf8_error_to_dicom_error(err, vr, offset))?
          .trim_matches(char::from(0))
          .trim()
          .to_string()
      ),
      "AT" => {
        let tmp: [u8; 4] = buffer[offset..offset + 4].try_into()?;
        DicomValue::AT(u32::from_le_bytes(tmp).try_into()?)
      },
      "CS" => DicomValue::CS(
        from_utf8(&buffer[offset..offset + length])
          .map_err(|err| utf8_error_to_dicom_error(err, vr, offset))?
          .trim_matches(char::from(0))
          .trim()
          .to_string()
      ),
      "DA" => DicomValue::DA(
        from_utf8(&buffer[offset..offset + length])
          .map_err(|err| utf8_error_to_dicom_error(err, vr, offset))?
          .trim_matches(char::from(0))
          .trim()
          .to_string()
      ),
      "DS" => DicomValue::DS(
        from_utf8(&buffer[offset..offset + length])
          .map_err(|err| utf8_error_to_dicom_error(err, vr, offset))?
          .trim_matches(char::from(0))
          .trim()
          .to_string()
      ),
      "DT" => DicomValue::DT(
        from_utf8(&buffer[offset..offset + length])
          .map_err(|err| utf8_error_to_dicom_error(err, vr, offset))?
          .trim_matches(char::from(0))
          .trim()
          .to_string()
      ),
      "FD" => {
        let tmp: [u8; 8] = buffer[offset..offset + 8].try_into()?;
        DicomValue::FD(f64::from_le_bytes(tmp))
      }
      "FL" => {
        let tmp: [u8; 4] = buffer[offset..offset + 4].try_into()?;
        DicomValue::FL(f32::from_le_bytes(tmp))
      }
      "IS" => DicomValue::IS(
        from_utf8(&buffer[offset..offset + length])
          .map_err(|err| utf8_error_to_dicom_error(err, vr, offset))?
          .trim_matches(char::from(0))
          .trim()
          .to_string()
      ),
      "LO" => DicomValue::LO(
        from_utf8(&buffer[offset..offset + length])
          .map_err(|err| utf8_error_to_dicom_error(err, vr, offset))?
          .trim_matches(char::from(0))
          .trim()
          .to_string()
      ),
      "LT" => DicomValue::LT(
        from_utf8(&buffer[offset..offset + length])
          .map_err(|err| utf8_error_to_dicom_error(err, vr, offset))?
          .trim_matches(char::from(0))
          .trim()
          .to_string()
      ),
      "OB" => DicomValue::OB(&buffer[offset..offset + length]),
      "OW" => {
        let (_, owslice, _) = unsafe {
          buffer[offset..offset + length].align_to::<u16>()
        };
        DicomValue::OW(owslice)
      },
      "PN" => DicomValue::PN(
        from_utf8(&buffer[offset..offset + length])
          .map_err(|err| utf8_error_to_dicom_error(err, vr, offset))?
          .trim_matches(char::from(0))
          .trim()
          .to_string()
      ),
      "SH" => DicomValue::SH(
        from_utf8(&buffer[offset..offset + length])
          .map_err(|err| utf8_error_to_dicom_error(err, vr, offset))?
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
          .map_err(|err| utf8_error_to_dicom_error(err, vr, offset))?
          .trim_matches(char::from(0))
          .trim()
          .to_string()
      ),
      "TM" => DicomValue::TM(
        from_utf8(&buffer[offset..offset + length])
          .map_err(|err| utf8_error_to_dicom_error(err, vr, offset))?
          .trim_matches(char::from(0))
          .trim()
          .to_string()
      ),
      "UI" => DicomValue::UI(
        from_utf8(&buffer[offset..offset + length])
          .map_err(|err| utf8_error_to_dicom_error(err, vr, offset))?
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
          .map_err(|err| utf8_error_to_dicom_error(err, vr, offset))?
          .trim_matches(char::from(0))
          .trim()
          .to_string()
      ),
      "UN" => DicomValue::UN(&buffer[offset..offset + length]),
      _ => unimplemented!("Value representation \"{}\" not implemented", vr),
    })
  }
}

#[derive(Debug)]
pub struct DicomAttribute<'a> {
  pub group: u16,
  pub element: u16,
  pub vr: Cow<'a, str>,
  // Position of the tag content in the buffer after the VR and length
  pub data_offset: usize,
  // Length of the data field. Always set to the correct length
  pub data_length: usize,
  // Length as read on the file
  pub length: usize,
  pub tag: Tag,
  pub subattributes: Vec<DicomAttribute<'a>>,
}

impl<'a> DicomAttribute<'a> {
  pub fn new<S>(group: u16, element: u16, vr: S, data_offset: usize, data_length: usize,
    length: usize, tag: Tag) -> DicomAttribute<'a>
    where S: Into<Cow<'a, str>> {
    Self::new_with_subattributes(group, element, vr.into(), data_offset, data_length, length, tag, vec![])
  }

  pub fn new_with_subattributes<S>(group: u16, element: u16, vr: S, data_offset: usize, data_length: usize,
    length: usize, tag: Tag, subattributes: Vec<DicomAttribute<'a>>) -> DicomAttribute<'a>
    where S: Into<Cow<'a, str>> {
    DicomAttribute { group, element, vr: vr.into(), data_offset, data_length, length, tag, subattributes }
  }
}

impl Instance {
  pub fn from_buf_reader<T: Read>(mut buf_reader: BufReader<T>) -> Result<Self, DicomError> {
    // Read the whole file into a buffer
    let mut buffer: Vec<u8> = vec![];
    buf_reader.read_to_end(&mut buffer)?;
    Instance::from(buffer)
  }

  pub fn from_filepath(filepath: &str) -> Result<Self, DicomError> {
    let f = File::open(filepath)?;
    return Instance::from_buf_reader(BufReader::new(f));
  }

  pub fn from(buffer: Vec<u8>) -> Result<Self, DicomError> {
    // Check it's a DICOM file
    // TODO: Manage headerless DICOM files
    if !has_dicom_header(&buffer) {
      return Err(DicomError::new("Not a DICOM file"));
    }

    let mut instance = Instance { buffer, implicit: false };

    match instance.is_supported_type() {
      Err(e) => Err(e),
      Ok(transfer_syntax_uid) => {
        instance.implicit = transfer_syntax_uid == "1.2.840.10008.1.2";
        Ok(instance)
      },
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

  pub fn get_value<'a>(self: &'a Self, tag: &Tag) -> Result<Option<DicomValue>, DicomError> {
    // Fast forward the DICOM prefix
    // TODO: Deal with non-comformant DICOM files
    // println!("get_value: {:?}", tag);
    let mut offset = 128 + "DICM".len();
    return loop {
      // println!("get_value: offset: {:#06x} buffer length: {:#06x}", offset, self.buffer.len());
      let field = self.next_attribute(offset)?;
      if field.group == tag.group && field.element == tag.element {
        break Ok(Some(DicomValue::from_dicom_attribute(&field, &self)?));
      }

      offset = field.data_offset + if field.data_length == 0xFFFFFFFF { 0 } else { field.data_length };
      if offset >= self.buffer.len() {
        break Ok(None)
      }
    };
  }

  pub fn iter(&self) -> InstanceIter<'_> {
    InstanceIter::new(self)
  }

  // https://dicom.nema.org/medical/dicom/current/output/chtml/part05/sect_A.4.html
  fn retrieve_next_data_element<'a>(self: &'a Self, offset: usize, items: &mut Vec<DicomAttribute>,
    item_length: &mut usize) -> Result<(), DicomError> {
    let original_offset = offset;
    let mut offset = offset;
    if offset >= self.buffer.len() {
      return Err(DicomError::new(
        &format!("Trying to read out of file bound (offset: {}, file size: {})", offset, self.buffer.len()))
      );
    }
    let group = self.buffer[offset] as u16 | (self.buffer[offset + 1] as u16) << 8;
    let element = self.buffer[offset + 2] as u16 | (self.buffer[offset + 3] as u16) << 8;
    offset += 4;
    if group == 0xFFFE && element == 0xE000 {
      let length = {
        let tmp: [u8; 4] = self.buffer[offset..offset + 4].try_into().unwrap();
        offset += 4;
        u32::from_le_bytes(tmp) as usize
      };
      let data: &[u32] = if length != 0 {
        let tmp: &[u8] = self.buffer[offset..offset + length].try_into().unwrap();
        offset += length;
        unsafe { std::mem::transmute(tmp) } // Basic Offset Table data is 32bits unsigned
      } else { &[] };

      let tag = (((group as u32) << 16) | element as u32).try_into().unwrap_or(Tag {
        group,
        element,
        name: "Unknown Tag & Data",
        vr: "",
        vm: std::ops::Range { start: 0, end: 0 },
        description: "Unknown Tag & Data",
      });
      items.push(DicomAttribute::new(group, element, "OB", offset, length, length, tag));
      self.retrieve_next_data_element(offset, items, item_length)?;
    }
    else if group == 0xFFFE && element == 0xE0DD {
      offset += 4;
      items.push(DicomAttribute::new(group, element, "", offset, 4, 4, SequenceDelimitationItem));
    } else {
      return Err(DicomError::new(&format!("Expecting sequence items in an ecapsulated pixel data field")));
    }

    *item_length += offset - original_offset;
    return Ok(());
  }

  // This "correct" the tag based on observed behavior in the wild.
  // TODO: The design of this function is questionable and force the use of a
  // mutable tag which I rather avoid. But its existence seems unavoidable due
  // to the broken nature of the implicit DICOM transfer syntax.
  fn get_implicit_vr<'a>(self: &'a Self, tag: &mut Tag) -> Result<(), DicomError> {
    // Finding the implicit VR is not straightforward. This is DICOM after all...
    // https://dicom.nema.org/medical/dicom/2017a/output/chtml/part05/chapter_A.html
    if tag.group == 0x7FE0 && tag.element == 0x0010 { // PixelData
      tag.vr = "OW";
      tag.name = "PixelData";
    }
    if tag.group == 0x0028 && tag.element == 0x0106 { // SmallestImagePixelValue
      // DICOM makes some fields' value representation depend on the value of other field AND
      // make these value respresentation implicit. What an awful mess...
      let unsigned = self.get_value(&0x00280103.try_into().unwrap())? == Some(DicomValue::US(0));
      if unsigned { tag.vr = "US" } else { tag.vr = "SS" };
      tag.name = "SmallestImagePixelValue";
    }
    if tag.group == 0x0028 && tag.element == 0x0107 { // LargestImagePixelValue
      let unsigned = self.get_value(&0x00280103.try_into().unwrap())? == Some(DicomValue::US(0));
      if unsigned { tag.vr = "US" } else { tag.vr = "SS" };
      tag.name = "LargestImagePixelValue";
    }
    if tag.element == 0x0000 { // GenericGroupLength
      // So apparently, all tag with element = 0 are GenericGroupLength.
      tag.vr = "UL";
      tag.name = "GenericGroupLength";
    }

    return Ok(());
  }

  pub fn next_attribute<'a>(self: &'a Self, offset: usize) -> Result<DicomAttribute<'a>, DicomError> {
    // group(u16),element(u16),vr(str[2]),length(u16)
    // println!("next_attribute: {:#04x?}", offset);
    let mut offset = offset;
    if offset >= self.buffer.len() {
      return Err(DicomError::new(
        &format!("Trying to read out of file bound (offset: {}, file size: {})", offset, self.buffer.len()))
      );
    }
    let group = self.buffer[offset] as u16 | (self.buffer[offset + 1] as u16) << 8;
    let element = self.buffer[offset + 2] as u16 | (self.buffer[offset + 3] as u16) << 8;
    // println!("next_attribute: {:#04x?} {:#06x?}:{:#06x?}", offset, group, element);
    offset += 4; // Skip group and element
    // Check if we have a sequence related data element
    if group == 0xFFFE {
      // Sequence delimiter items can have a length or 0xFFFFFFFF like sequence themselves
      let tmp: [u8; 4] = self.buffer[offset..offset + 4].try_into().unwrap();
      let length = u32::from_le_bytes(tmp) as usize; // Can sometimes be equal to 0xFFFFFFFF
      offset += 4;
      return match element {
        0xE000 => {
          let mut subattributes: Vec<DicomAttribute> = vec![];
          let mut subattribute;
          let mut suboffset = offset;
          let mut item_length = 0;
          while suboffset < (offset + length) {
            subattribute = self.next_attribute(suboffset)?;
            suboffset = subattribute.data_offset + subattribute.data_length;
            item_length = (subattribute.data_offset + subattribute.data_length) - offset;
            if subattribute.tag == ItemDelimitationItem {
              break;
            }
            subattributes.push(subattribute);
          }
          Ok(DicomAttribute::new_with_subattributes(group, element, "", offset, item_length,
            length, Item, subattributes))
        }
        0xE00D => Ok(DicomAttribute::new(group, element, "", offset, length, length, ItemDelimitationItem)),
        0xE0DD => Ok(DicomAttribute::new(group, element, "", offset, length, length, SequenceDelimitationItem)),
        _ => Err(DicomError::new(&format!("unknown sequence related data element: {}", element))),
      }
    }
    // Create tag based on group and element or generate a synthetic "unknown" tag
    let mut tag = (((group as u32) << 16) | element as u32).try_into().unwrap_or(Tag {
      group,
      element,
      name: "Unknown Tag & Data",
      vr: "UN",
      vm: std::ops::Range { start: 0, end: 0 },
      description: "Unknown Tag & Data",
    });
    let vr = if group == 0x0002 || !self.implicit {
      offset += 2; // Skip VR
      from_utf8(&self.buffer[offset - 2..offset])
        .map_err(|err| utf8_error_to_dicom_error(err, "tag", offset - 2))?
    } else {
      self.get_implicit_vr(&mut tag)?;
      tag.vr
    };

    let length: usize;
    if ["OB", "OD", "OF", "OL", "OW", "SQ", "UC", "UR", "UT", "UN"].contains(&vr) {
      // These VR types handles themselves differently. They have 2 reserved bytes
      // that need to be skipped and their data length is on 4 bytes.
      // https://dicom.nema.org/dicom/2013/output/chtml/part05/chapter_7.html#sect_7.1.2
      if group == 0x0002 || !self.implicit {
        offset += 2; // Skip reserved byte
      }
      let tmp: [u8; 4] = self.buffer[offset..offset + 4].try_into().unwrap();
      length = u32::from_le_bytes(tmp) as usize; // Can sometimes be equal to 0xFFFFFFFF
      offset += 4;
      if vr == "SQ" || // Sequence are special types within those special types... yikes.
         length == 0xFFFFFFFF { // https://github.com/pydicom/pydicom/issues/1140
        let mut items: Vec<DicomAttribute> = vec![];
        let mut item_length = 0;
        if group == 0x7FE0 && element == 0x0010 {
          // https://dicom.nema.org/medical/dicom/current/output/chtml/part05/sect_A.4.html
          // We treat here the case of encapsulated pixel data
          // return Err(DicomError::new(&format!("Encapsulated pixel data not supported")));
          self.retrieve_next_data_element(offset, &mut items, &mut item_length)?;
        } else {
          // See http://dicom.nema.org/dicom/2013/output/chtml/part05/sect_7.5.html
          // on what a sequence look like in a DICOM file.
          let mut item;
          let mut suboffset = offset;
          // Go through the items in the sequence and fetch them recursively
          while suboffset < (offset + length) {
            item = self.next_attribute(suboffset)?;
            suboffset = item.data_offset + item.data_length;
            item_length = (item.data_offset + item.data_length) - offset;
            if item.tag == SequenceDelimitationItem {
              break;
            }
            items.push(item);
          }
        }
        return Ok(DicomAttribute::new_with_subattributes(
          group, element, vr, offset, item_length, length, tag, items,
        ));
      }
    } else {
      length = if group == 0x0002 || !self.implicit {
        offset = offset + 2;
        self.buffer[offset - 2] as usize | (self.buffer[offset - 1] as usize) << 8
      } else {
        offset = offset + 4;
        self.buffer[offset - 4] as usize |
          (self.buffer[offset - 3] as usize) << 8 |
          (self.buffer[offset - 2] as usize) << 16 |
          (self.buffer[offset - 1] as usize) << 24
      }
    }
    Ok(DicomAttribute::new(group, element, vr, offset, length, length, tag))
  }

  fn is_supported_type(self: &Self) -> Result<String, DicomError> {
    // Only supporting little-endian explicit VR for now.
    if let Some(transfer_syntax_uid_field) = self.get_value(&0x00020010.try_into().unwrap())? {
      match transfer_syntax_uid_field {
        DicomValue::UI(transfer_syntax_uid) => {
          if !vec![
            // "1.2.840.10008.1.2",      // Implicit VR Little Endian: Default Transfer Syntax for DICOM
            "1.2.840.10008.1.2.1.99", // Deflated Explicit VR Little Endian
            "1.2.840.10008.1.2.2",    // Explicit VR Big Endian
            ].contains(&transfer_syntax_uid.as_str()) {
            Ok(transfer_syntax_uid)
          }
          else {
            Err(DicomError::new(
              &format!("Unsupported Transfer Syntax UID: {} ({})",
                transfer_syntax_uid,
                get_transfer_syntax_uid_label(&transfer_syntax_uid).unwrap_or("Unknown transfer syntax uid"))))
          }
        }
        _ => Err(DicomError::new(&format!("Unexpected type")))
      }
    } else {
      Err(DicomError::new("Transfer Syntax UID not found"))
    }
  }
}

fn get_transfer_syntax_uid_label(transfer_syntax_uid: &str) -> Result<&str, DicomError> {
  match transfer_syntax_uid {
    "1.2.840.10008.1.2" => Ok("Implicit VR Little Endian: Default Transfer Syntax for DICOM"),
    "1.2.840.10008.1.2.1" => Ok("Explicit VR Little Endian"),
    "1.2.840.10008.1.2.1.99" => Ok("Deflated Explicit VR Little Endian"),
    "1.2.840.10008.1.2.2" => Ok("Explicit VR Big Endian"),
    "1.2.840.10008.1.2.4.50" => Ok("JPEG Baseline (Process 1)"),
    "1.2.840.10008.1.2.4.51" => Ok("JPEG Baseline (Processes 2 & 4)"),
    "1.2.840.10008.1.2.4.52" => Ok("JPEG Extended (Processes 3 & 5)"),
    "1.2.840.10008.1.2.4.53" => Ok("JPEG Spectral Selection, Nonhierarchical (Processes 6 & 8)"),
    "1.2.840.10008.1.2.4.54" => Ok("JPEG Spectral Selection, Nonhierarchical (Processes 7 & 9)"),
    "1.2.840.10008.1.2.4.55" => Ok("JPEG Full Progression, Nonhierarchical (Processes 10 & 12)"),
    "1.2.840.10008.1.2.4.56" => Ok("JPEG Full Progression, Nonhierarchical (Processes 11 & 13)"),
    "1.2.840.10008.1.2.4.57" => Ok("JPEG Lossless, Nonhierarchical (Processes 14)"),
    "1.2.840.10008.1.2.4.58" => Ok("JPEG Lossless, Nonhierarchical (Processes 15)"),
    "1.2.840.10008.1.2.4.59" => Ok("JPEG Extended, Hierarchical (Processes 16 & 18)"),
    "1.2.840.10008.1.2.4.60" => Ok("JPEG Extended, Hierarchical (Processes 17 & 19)"),
    "1.2.840.10008.1.2.4.61" => Ok("JPEG Spectral Selection, Hierarchical (Processes 20 & 22)"),
    "1.2.840.10008.1.2.4.62" => Ok("JPEG Spectral Selection, Hierarchical (Processes 21 & 23)"),
    "1.2.840.10008.1.2.4.63" => Ok("JPEG Full Progression, Hierarchical (Processes 24 & 26)"),
    "1.2.840.10008.1.2.4.64" => Ok("JPEG Full Progression, Hierarchical (Processes 25 & 27)"),
    "1.2.840.10008.1.2.4.65" => Ok("JPEG Lossless, Nonhierarchical (Process 28)"),
    "1.2.840.10008.1.2.4.66" => Ok("JPEG Lossless, Nonhierarchical (Process 29)"),
    "1.2.840.10008.1.2.4.70" => Ok("JPEG Lossless, Nonhierarchical, First- Order Prediction (Processes 14 [Selection Value 1])"),
    "1.2.840.10008.1.2.4.80" => Ok("JPEG-LS Lossless Image Compression"),
    "1.2.840.10008.1.2.4.81" => Ok("JPEG-LS Lossy (Near- Lossless) Image Compression"),
    "1.2.840.10008.1.2.4.90" => Ok("JPEG 2000 Image Compression (Lossless Only)"),
    "1.2.840.10008.1.2.4.91" => Ok("JPEG 2000 Image Compression"),
    "1.2.840.10008.1.2.4.92" => Ok("JPEG 2000 Part 2 Multicomponent Image Compression (Lossless Only)"),
    "1.2.840.10008.1.2.4.93" => Ok("JPEG 2000 Part 2 Multicomponent Image Compression"),
    "1.2.840.10008.1.2.4.94" => Ok("JPIP Referenced"),
    "1.2.840.10008.1.2.4.95" => Ok("JPIP Referenced Deflate"),
    "1.2.840.10008.1.2.5" => Ok("RLE Lossless"),
    "1.2.840.10008.1.2.6.1" => Ok("RFC 2557 MIME Encapsulation"),
    "1.2.840.10008.1.2.4.100" => Ok("MPEG2 Main Profile Main Level"),
    "1.2.840.10008.1.2.4.102" => Ok("MPEG-4 AVC/H.264 High Profile / Level 4.1"),
    "1.2.840.10008.1.2.4.103" => Ok("MPEG-4 AVC/H.264 BD-compatible High Profile / Level 4.1"),
    _ => Err(DicomError::new(&format!("Unknown transfer_syntax_uid: {}", transfer_syntax_uid)))
  }
}

pub struct InstanceIter<'a> {
  instance: &'a Instance,
  offset: usize,
}

impl<'a> InstanceIter<'a> {
  fn new(instance: &'a Instance) -> Self {
    // TODO: Deal with DICOM with broken headers
    return InstanceIter { instance: instance, offset: 128 + "DICM".len() };
  }
}

impl<'a> Iterator for InstanceIter<'a> {
  type Item = Result<DicomAttribute<'a>, DicomError>;

  fn next(&mut self) -> std::option::Option<<Self as Iterator>::Item> {
    if self.offset < self.instance.buffer.len() {
      match self.instance.next_attribute(self.offset) {
        Ok(attribute) => {
          self.offset = attribute.data_offset + attribute.data_length;
          Some(Ok(attribute))
        },
        Err(e) => Some(Err(e)),
      }
    } else { None }
  }
}

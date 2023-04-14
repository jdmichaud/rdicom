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

use rdicom::error::DicomError;
use rdicom::instance::DicomAttribute;
use std::error::Error;
use std::io::BufReader;
use std::fs::File;
use std::io::{self};

use structopt::StructOpt;
use structopt::clap::AppSettings;

use rdicom::instance::Instance;
use rdicom::misc::is_dicom_file;
use rdicom::instance::DicomValue;
use rdicom::dicom_tags::{Item, SequenceDelimitationItem};

/// A dcmdump clone based on rdicom
#[derive(Debug, StructOpt)]
#[structopt(
  name = format!("dump {} ({} {})", env!("GIT_HASH"), env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")),
  no_version,
  global_settings = &[AppSettings::DisableVersion]
)]
struct Opt {
  /// DICOM input file to be dumped
  filepath: String,
}

fn get_tag_sequence<'b, 'a>(instance: &'a Instance, field: &DicomAttribute<'a>, level: usize)
//       (group, element, vr,     value,  length, multiplicity, tag_name, level)
  -> Vec<(u16,   u16,     String, String, String, usize,        &'a str,  usize)> {
  let mut result: Vec<(u16, u16, String, String, String, usize, &'a str, usize)> = vec![];
  match field.vr.as_ref() {
    "SQ" => {
      result.push((field.group, field.element, String::from("SQ"),
        if field.length == 0xFFFFFFFF {
          format!("(Sequence with undefined length #={})", field.subattributes.len())
        } else {
          format!("(Sequence with explicit length #={})", field.subattributes.len())
        },
        if field.length == 0xFFFFFFFF { "u/l".to_string() } else { format!("{}", field.length) },
        1, field.tag.name, level,
      ));
      result.append(&mut field.subattributes.iter()
        .map(|attr| get_tag_sequence(instance, &attr, level + 1))
        .flatten()
        .collect::<_>()
      );
      result.push((0xFFFE, 0xE0DD, String::from("na"),
        if field.length != 0xFFFFFFFF {
          "(SequenceDelimitationItem for re-encod.)".to_string()
        } else {
          "(SequenceDelimitationItem)".to_string()
        },
        format!("{}", 0), 0, "SequenceDelimitationItem", level,
      ));
    },
    _ if field.group == Item.group && field.element == Item.element => {
      let mut sequence_tags: Vec<_> = field.subattributes.iter()
        .map(|attr| get_tag_sequence(instance, &attr, level + 1))
        .flatten()
        .collect::<_>();
      result.push((field.group, field.element, String::from("na"),
        if field.length == 0xFFFFFFFF as usize {
          format!("(Item with undefined length #={})", field.subattributes.len())
        } else {
          format!("(Item with explicit length #={})", field.subattributes.len())
        },
        if field.length == 0xFFFFFFFF { "u/l".to_string() } else { format!("{}", field.length) },
        1, field.tag.name, level,
      ));
      result.append(&mut sequence_tags);
      result.push((0xFFFE, 0xE00D, String::from("na"),
        if field.length != 0xFFFFFFFF {
          "(ItemDelimitationItem for re-encoding)".to_string()
        } else {
          "(ItemDelimitationItem)".to_string()
        }, format!("{}", 0), 0, "ItemDelimitationItem", level,
      ));
    },
    _ if field.group == SequenceDelimitationItem.group && field.element == SequenceDelimitationItem.element => {
      result.push((field.group, field.element, String::from("na"),
        "(SequenceDelimitationItem)".to_string(), "0".to_string(), 0, field.tag.name, level));
      return result;
    },
    _ => {
      let value = DicomValue::from_dicom_attribute(&field, &instance).unwrap();
      match value {
        DicomValue::UI(payload) => {
          let mut display_value = payload.to_string();
          if display_value.len() > 66 {
            display_value.replace_range(66.., "...");
          }
          let (display_value, multiplicity) = if display_value == "" {
            ("(no value available)".to_string(), 0)
          } else {
            (format!("[{}]", display_value), 1)
          };
          result.push((field.group, field.element, field.vr.to_string(), display_value,
            format!("{}", field.data_length), multiplicity, field.tag.name, level));
        },
        DicomValue::AE(payload) |
        DicomValue::AS(payload) |
        DicomValue::DA(payload) |
        DicomValue::IS(payload) |
        DicomValue::LO(payload) |
        DicomValue::LT(payload) |
        DicomValue::PN(payload) |
        DicomValue::SH(payload) |
        DicomValue::ST(payload) |
        DicomValue::TM(payload) |
        DicomValue::DT(payload) |
        DicomValue::CS(payload) |
        DicomValue::UT(payload) |
        DicomValue::DS(payload) => {
          let mut display_value = payload.join("\\");
          if display_value.len() > 66 {
            display_value.replace_range(66.., "...");
          }
          let (display_value, multiplicity) = if display_value == "" {
            ("(no value available)".to_string(), 0)
          } else {
            (format!("[{}]", display_value), payload.len())
          };
          result.push((field.group, field.element, field.vr.to_string(), display_value,
            format!("{}", field.data_length), multiplicity, field.tag.name, level));
        },
        DicomValue::SeqItemEnd => {
          result.push((field.group, field.element, String::from("na"),
            "(ItemDelimitationItem)".to_string(), "u/l".to_string(), 1, "Item", level,
          ));
          return result;
        },
        DicomValue::SeqEnd => {
          panic!("Unexpected SeqEnd");
        },
        DicomValue::FD(payload) => {
          let display_value = value.to_string();
          let (display_value, multiplicity) = if display_value == "" {
            ("(no value available)".to_string(), 0)
          } else {
            (display_value, payload.len())
          };
          result.push((field.group, field.element, field.vr.to_string(), display_value,
            format!("{}", field.data_length), multiplicity, field.tag.name, level));
        },
        DicomValue::FL(payload) => {
          let display_value = value.to_string();
          let (display_value, multiplicity) = if display_value == "" {
            ("(no value available)".to_string(), 0)
          } else {
            (display_value, payload.len())
          };
          result.push((field.group, field.element, field.vr.to_string(), display_value,
            format!("{}", field.data_length), multiplicity, field.tag.name, level));
        },
        _ => {
          let display_value = value.to_string();
          let (display_value, multiplicity) = if display_value == "" {
            ("(no value available)".to_string(), 0)
          } else {
            (display_value, 1)
          };
          result.push((field.group, field.element, field.vr.to_string(), display_value,
            format!("{}", field.data_length), multiplicity, field.tag.name, level));
        },
      }
    }
  };
  result
}

fn dump(opt: &Opt) -> Result<(), DicomError> {
  let f = File::open(&opt.filepath)?;

  if is_dicom_file(&opt.filepath) {
    let instance = Instance::from_buf_reader(BufReader::new(f))?;
    println!("");
    println!("# Dicom-File-Format");
    println!("");

    println!("# Dicom-Meta-Information-Header");
    println!("# Used TransferSyntax: Little Endian Explicit");

    let mut offset = 128 + "DICM".len();
    let mut header = true;

    let mut tags = vec![];
    while offset < instance.buffer.len() {
      let attribute = &instance.next_attribute(offset)?;
      tags.append(&mut get_tag_sequence(&instance, attribute, 0));
      offset = attribute.data_offset + attribute.data_length;
    }

    for (group, element, vr, value, length, multiplicity, tag_name, level) in tags {
      if header && group > 0x0002 {
        header = false;
        println!("");
        println!("# Dicom-Data-Set");
        println!("# Used TransferSyntax: Little Endian {}", if instance.implicit { "Implicit" } else { "Explicit" });
      }
      println!("{}({:04x},{:04x}) {} {: <40} # {: >3},{: >2} {}",
        " ".repeat(level * 2), group, element, vr, value, length, multiplicity, tag_name);
    }
  }
  Ok(())
}

fn main() {
  let opt = Opt::from_args();
  if let Err(e) = dump(&opt) {
    eprintln!("error: {}", e.details);
    std::process::exit(1)
  }
}

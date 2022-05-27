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

fn get_tag_sequence<'b, 'a>(instance: &'a Instance, offset: &'b mut usize, upto: usize, level: usize)
//       (group, element, vr, value, length, multiplicity, tag_name, level)
  -> Vec<(u16, u16, String, String, String, usize, &'a str, usize)> {
  let mut result: Vec<(u16, u16, String, String, String, usize, &'a str, usize)> = vec![];
  while (upto == 0 && *offset < instance.buffer.len()) || *offset < upto {
    let field = instance.next_attribute(*offset).unwrap();
    match field.vr.as_ref() {
      "SQ" => {
        *offset = field.offset;
        let mut sequence_tags = get_tag_sequence(instance, offset,
          if field.length == 0xFFFFFFF { 0 } else { *offset + field.length }, level + 1);
        result.push((field.group, field.element, String::from("SQ"),
          if field.length == 0xFFFFFFF {
            format!("(Sequence with undefined length #={})", sequence_tags.len())
          } else {
            format!("(Sequence with explicit length #={})", sequence_tags.len())
          },
          if field.length == 0xFFFFFFF { "u/l".to_string() } else { format!("{}", field.length) },
          1, field.tag.name, level,
        ));
        result.append(&mut sequence_tags);
        if field.length != 0xFFFFFFF {
          result.push((0xFFFE, 0xE0DD, String::from("na"),
            "(SequenceDelimitationItem for re-encod.)".to_string(), format!("{}", 0), 0,
            "SequenceDelimitationItem", level,
          ));
        }
      }
      _ if field.group == SequenceDelimitationItem.group && field.element == SequenceDelimitationItem.element => {
        *offset = field.offset;
        result.push((field.group, field.element, String::from("na"),
          "(SequenceDelimitationItem)".to_string(), "0".to_string(), 0, field.tag.name, level));
        return result;
      }
      _ => {
        *offset = field.offset + field.length;
        let value = DicomValue::from_dicom_field(&field, &instance);
        match value {
          DicomValue::AE(value) |
          DicomValue::AS(value) |
          DicomValue::DA(value) |
          DicomValue::IS(value) |
          DicomValue::LO(value) |
          DicomValue::LT(value) |
          DicomValue::PN(value) |
          DicomValue::SH(value) |
          DicomValue::ST(value) |
          DicomValue::TM(value) |
          DicomValue::UT(value) => {
            let mut display_value = value.to_string();
            if display_value.len() > 66 {
              display_value.replace_range(66.., "...");
            }
            let (display_value, multiplicity) = if display_value == "" {
              ("(no value available)".to_string(), 0)
            } else {
              (format!("[{}]", display_value), 1)
            };
            result.push((field.group, field.element, field.vr.to_string(), display_value,
              format!("{}", field.length), multiplicity, field.tag.name, level));
          },
          DicomValue::CS(value) |
          DicomValue::DS(value) => {
            let mut display_value = value.to_string();
            if display_value.len() > 66 {
              display_value.replace_range(66.., "...");
            }
            let (display_value, multiplicity) = if display_value == "" {
              ("(no value available)".to_string(), 0)
            } else {
              (format!("[{}]", display_value), display_value.matches("\\").count() + 1)
            };
            result.push((field.group, field.element, field.vr.to_string(), display_value,
              format!("{}", field.length), multiplicity, field.tag.name, level));
          },
          DicomValue::SeqItem => {
            *offset -= field.length;
            let mut sequence_tags = get_tag_sequence(instance, offset,
              if field.length == 0xFFFFFFF { 0 } else { *offset + field.length }, level + 1);
            result.push((field.group, field.element, String::from("na"),
              if field.length == 0xFFFFFFF {
                format!("(Item with undefined length #={})", sequence_tags.len())
              } else {
                format!("(Item with explicit length #={})", sequence_tags.len())
              },
              if field.length == 0xFFFFFFF { "u/l".to_string() } else { format!("{}", field.length) },
              1, field.tag.name, level,
            ));
            result.append(&mut sequence_tags);
            if field.length != 0xFFFFFFF {
              result.push((0xFFFE, 0xE00D, String::from("na"),
                "(ItemDelimitationItem for re-encoding)".to_string(), format!("{}", 0), 0,
                "ItemDelimitationItem", level,
              ));
            }
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
          _ => {
            let display_value = value.to_string();
            let (display_value, multiplicity) = if display_value == "" {
              ("(no value available)".to_string(), 0)
            } else {
              (display_value, 1)
            };
            result.push((field.group, field.element, field.vr.to_string(), display_value,
              format!("{}", field.length), multiplicity, field.tag.name, level));
          },
        }
      }
    };
  }

  result
}

fn main() -> Result<(), Box<dyn Error>> {
  let opt = Opt::from_args();
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

    let tags = get_tag_sequence(&instance, &mut offset, 0, 0);
    for (group, element, vr, value, length, multiplicity, tag_name, level) in tags {
      if header && group != 2 {
        header = false;
        println!("");
        println!("# Dicom-Data-Set");
        println!("# Used TransferSyntax: Little Endian Explicit");
      }
      println!("{}({:04x},{:04x}) {} {: <40} # {: >3},{: >2} {}",
        " ".repeat(level * 2), group, element, vr, value, length, multiplicity, tag_name);
    }
  }
  Ok(())
}

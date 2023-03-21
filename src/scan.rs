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

use std::collections::{HashMap, HashSet};
use std::io::BufReader;
use std::fs::File;
use std::path::Path;
use std::fs::metadata;
use std::path::PathBuf;
use std::error::Error;
use structopt::StructOpt;
use walkdir::WalkDir;
use std::io::{self, Write};
use sqlite::{Connection};
use serde_yaml;

use rdicom::dicom_tags;
use rdicom::instance::Instance;
use rdicom::misc::is_dicom_file;

mod config;

const ESC: char = 27u8 as char;
const MEDIA_STORAGE_DIRECTORY_STORAGE: &str = "1.2.840.10008.1.3.10";

fn path_is_folder(path: &str) -> Result<PathBuf, Box<dyn Error>> {
  let path_buf = PathBuf::from(path);
  if !path_buf.exists() {
    return Err(format!("{} does not exists", path).into());
  }
  let metadata = metadata(path)?;
  if !metadata.is_dir() {
    return Err(format!("{} is not a folder", path).into());
  }
  Ok(path_buf)
}

fn file_exists(path: &str) -> Result<PathBuf, Box<dyn Error>> {
    let path_buf = PathBuf::from(path);
    if path_buf.exists() {
        Ok(path_buf)
    } else {
        Err(format!("{} does not exists", path).into())
    }
}

#[derive(Debug, StructOpt)]
/// Scan a folder for DICOM assets and create an index file in CSV or SQL format.
struct Opt {
    /// YAML configuration file containing the list of files to be indexed from the DICOM assets.
    #[structopt(short, long, parse(try_from_str = file_exists))]
    config: PathBuf,
    /// Path to a folder containing DICOM assets. Will be scanned recursively.
    #[structopt(short, long, parse(try_from_str = path_is_folder))]
    input_path: PathBuf,
    /// CSV output file
    #[structopt(long)]
    csv_output: Option<PathBuf>,
    /// SQL output file
    #[structopt(long)]
    sql_output: Option<PathBuf>,
}

trait IndexStore {
  fn write(&mut self, data: &HashMap<String, String>) -> Result<(), Box<dyn Error>>;
}

#[derive(Debug)]
struct CsvIndexStore<W: Write> {
  writer: W,
  fields: Vec<String>
}

impl<W: Write> CsvIndexStore<W> {
  fn new(mut writer: W, mut fields: Vec<String>) -> Self {
    fields.push("filepath".to_string());
    let header = fields.iter().map(|s| String::from("\"") + s + "\"").collect::<Vec<String>>().join(",");
    writeln!(writer, "").unwrap();
    CsvIndexStore { writer, fields }
  }
}

impl<W: Write> IndexStore for CsvIndexStore<W> {
  fn write(self: &mut Self, data: &HashMap<String, String>) -> Result<(), Box<dyn Error>> {
    for field in &self.fields {
      match write!(self.writer, "\"{}\",", data.get(field).unwrap_or(&"undefined".to_string())) {
        Ok(_) => (),
        Err(e) => return Err(Box::new(e)),
      }
    }
    writeln!(self.writer, "")?;
    Ok(())
  }
}

struct SqlIndexStore {
  connection: Connection,
  table_name: String,
  fields: Vec<String>
}

impl SqlIndexStore {
  fn new(filepath: &str, table_name: &str, mut fields: Vec<String>) -> Result<Self, Box<dyn Error>> {
    fields.push("filepath".to_string());
    let table = fields.iter()
      .map(|s| s.to_string() + " TEXT NON NULL")
      .collect::<Vec<String>>().join(",");
    let connection = Connection::open(filepath)?;
    connection.execute(&format!("CREATE TABLE IF NOT EXISTS {} ({});", table_name, table))?;
    Ok(SqlIndexStore { connection, table_name: String::from(table_name), fields })
  }
}

impl IndexStore for SqlIndexStore {
  fn write(self: &mut Self, data: &HashMap<String, String>) -> Result<(), Box<dyn Error>> {
    let values: Vec<_> = self.fields.iter()
      .map(|x| data.get(x).unwrap_or(&"undefined".to_owned()).clone())
      .map(|x| format!("\"{}\"", x))
      .collect::<Vec<String>>();
    let column_names = self.fields.join(",");
    let placeholders = (1..self.fields.len() + 1)
      .map(|i| format!("?{}", i))
      .collect::<Vec<String>>()
      .join(",");
    let query = &format!("INSERT INTO {} ({}) VALUES ({})",
      self.table_name, column_names, values.join(","));
    self.connection.execute(query)?;
    Ok(())
  }
}

fn walk(input_path: &str, f: &mut dyn FnMut(&Path) -> ()) -> Result<(), Box<dyn Error>> {
  for result in WalkDir::new(input_path) {
    let entry = result?;
    let filepath = entry.path();
    if filepath.is_file() {
      f(filepath);
    }
  }
  Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
  // Retrieve options
  let opt = Opt::from_args();
  // Load the config
  let config_file = std::fs::read_to_string(&opt.config)?;
  let config: config::Config = serde_yaml::from_str(&config_file)?;
  // Create an vector of fields to write in the index
  let mut indexable_fields = config.indexing.fields.series.into_iter().chain(
    config.indexing.fields.studies.into_iter().chain(
      config.indexing.fields.instances.into_iter(),
    ),
  ).collect::<Vec<String>>();
  // Create an index store depending on the options
  let mut index_store = if let Some(sql_output) = opt.sql_output {
    let filepath = &sql_output.to_string_lossy().to_string();
    Box::new(SqlIndexStore::new(filepath, &config.table_name, indexable_fields.to_vec()).unwrap()) as Box<dyn IndexStore>
  } else {
    let writer = if let Some(csv_output) = opt.csv_output {
      Box::new(File::create(csv_output)?) as Box<dyn Write>
    } else {
      Box::new(io::stdout()) as Box<dyn Write>
    };
    Box::new(CsvIndexStore::new(writer, indexable_fields.to_vec())) as Box<dyn IndexStore>
  };
  // There sets will be used for a fancy display
  let mut count = 0;
  let mut study_set: HashSet<String> = HashSet::new();
  let mut series_set: HashSet<String> = HashSet::new();
  let mut modality_set: HashSet<String> = HashSet::new();
  // Walk all the files in the provided input folder
  let _ = walk(&opt.input_path.to_string_lossy(), &mut move |filepath: &Path| {
    count += 1;
    // For each file, check it is a dicom file, load it and parse the requested fields
    if is_dicom_file(&filepath.to_string_lossy()) {
      match Instance::from_filepath(&filepath.to_string_lossy().to_string()) {
        Ok(instance) => {
          match instance.get_value(&"MediaStorageSOPClassUID".try_into().unwrap()) {
            // Ignore DICOMDIR files
            Ok(Some(sop_class_uid)) if sop_class_uid.to_string() != MEDIA_STORAGE_DIRECTORY_STORAGE => {
              let mut data = HashMap::<String, String>::new();
              // We want the filepath in the index by default
              data.insert("filepath".to_string(), filepath.to_string_lossy().to_string());
              for field in indexable_fields.iter() {
                match instance.get_value(&field.try_into().unwrap()) {
                  Ok(result) => {
                    let value = if let Some(value) = result { value.to_string() } else { "undefined".to_string() };
                    // Fill the hash map with the requested field
                    data.insert(field.to_string(), value);
                  }
                  Err(e) => {
                    print!("\r\x1b[2K");
                    io::stdout().flush().unwrap();
                    eprintln!("{}: {:?}", filepath.to_string_lossy(), e);
                  }
                }
              }
              // Provide the hash map to the index store
              index_store.write(&data).unwrap();

              // Fancy display
              if let Some(study_instance_uid) = data.get("StudyInstanceUID") {
                study_set.insert(study_instance_uid.clone());
              }
              if let Some(series_instance_uid) = data.get("SeriesInstanceUID") {
                series_set.insert(series_instance_uid.clone());
              }
              if let Ok(Some(modality)) = instance.get_value(&"Modality".try_into().unwrap()) {
                modality_set.insert(modality.to_string().clone());
              }
              let wheel = "-\\|/";
              let w = wheel.as_bytes()[count / 10 % 4] as char;
              print!("{} [{}] files scanned with [{}] studies and [{}] series found with following modalities {:?}\r",
                w, count, study_set.len(), series_set.len(), modality_set);
              io::stdout().flush().unwrap();
            },
            _ => (),
          }
        },
        Err(e) => {
          print!("\r\x1b[2K");
          io::stdout().flush().unwrap();
          eprintln!("{:?}", e);
        }
      }
    }
  });
  Ok(())
}

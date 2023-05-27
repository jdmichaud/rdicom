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

use serde_yaml;
use sqlite::{Connection, ConnectionWithFullMutex};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fs::metadata;
use std::fs::File;
use std::io::BufReader;
use std::io::{self, Write};
use std::path::Path;
use std::path::PathBuf;
use structopt::clap::AppSettings;
use structopt::StructOpt;
use walkdir::WalkDir;

use rdicom::dicom_tags;
use rdicom::dicom_tags::{MediaStorageSOPClassUID, Modality};
use rdicom::instance::Instance;
use rdicom::misc::is_dicom_file;

mod config;
mod db;
mod index_store;

use index_store::{CsvIndexStore, IndexStore, SqlIndexStore, SqlIndexStoreWithMutex};

const ESC: char = 27u8 as char;
const MEDIA_STORAGE_DIRECTORY_STORAGE: &str = "1.2.840.10008.1.3.10";

/// Scan a folder for DICOM assets and create an index file in CSV or SQL format.
#[derive(Debug, StructOpt)]
#[structopt(
  name = format!("scan {} ({} {})", env!("GIT_HASH"), env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")),
  no_version,
  global_settings = &[AppSettings::DisableVersion]
)]
struct Opt {
  /// YAML configuration file containing the list of files to be indexed from the DICOM assets.
  #[structopt(short, long, parse(try_from_str = file_exists))]
  config: PathBuf,
  /// CSV output file
  #[structopt(long)]
  csv_output: Option<PathBuf>,
  /// SQL output file
  #[structopt(long)]
  sql_output: Option<String>,
  /// Path to a folder containing DICOM assets. Will be scanned recursively.
  input_path: PathBuf,
  /// Log each files being scan on standard output
  #[structopt(short, long)]
  log_files: bool,
  /// Do not use a transaction on the database during the scan (slower but scan
  /// can be interrupted)
  #[structopt(short, long)]
  no_transaction: bool,
}

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

fn main() -> Result<(), Box<dyn Error>> {
  // Retrieve options
  let opt = Opt::from_args();
  // Load the config
  let config_file = std::fs::read_to_string(&opt.config)?;
  let config: config::Config = serde_yaml::from_str(&config_file)?;
  // Create an vector of fields to write in the index
  let indexable_fields = config
    .indexing
    .fields
    .series
    .into_iter()
    .chain(
      config
        .indexing
        .fields
        .studies
        .into_iter()
        .chain(config.indexing.fields.instances.into_iter()),
    )
    .collect::<Vec<String>>();
  // Create an index store depending on the options
  let mut index_store: Box<dyn IndexStore> = if let Some(sql_output) = opt.sql_output {
    let connection = Connection::open(sql_output)?;
    Box::new(SqlIndexStore::new(
      connection,
      &config.table_name,
      [indexable_fields.clone(), vec!["filepath".to_string()]].concat(),
    )?)
  } else {
    let writer: Box<dyn Write> = if let Some(csv_output) = opt.csv_output {
      Box::new(File::create(csv_output)?)
    } else {
      Box::new(io::stdout())
    };
    Box::new(CsvIndexStore::new(
      writer,
      [indexable_fields.clone(), vec!["filepath".to_string()]].concat(),
    ))
  };
  // There sets will be used for a fancy display
  let mut count = 0;
  let mut error_count = 0;
  let mut study_set: HashSet<String> = HashSet::new();
  let mut series_set: HashSet<String> = HashSet::new();
  let mut modality_set: HashSet<String> = HashSet::new();
  let path_prefix = opt.input_path.clone();
  if !opt.no_transaction {
    index_store.begin_transaction()?;
  }
  // Walk all the files in the provided input folder
  for result in WalkDir::new(opt.input_path) {
    let entry = result?;
    let filepath = entry.path();
    if filepath.is_file() {
      count += 1;
      // For each file, check it is a dicom file, load it and parse the requested fields
      if is_dicom_file(&filepath.to_string_lossy()) {
        let filepathstr = filepath.to_string_lossy().to_string();
        let relative_filepath_str = filepath
          .strip_prefix(path_prefix.clone())?
          .to_string_lossy()
          .to_string();
        match Instance::from_filepath(&filepathstr) {
          Ok(instance) => {
            if opt.log_files {
              println!("{}", filepathstr);
            }
            match instance.get_value(&MediaStorageSOPClassUID) {
              // Ignore DICOMDIR files
              Ok(Some(sop_class_uid))
                if sop_class_uid.to_string() != MEDIA_STORAGE_DIRECTORY_STORAGE =>
              {
                let mut data = HashMap::<String, String>::new();
                // We want the filepath in the index by default
                data.insert("filepath".to_string(), relative_filepath_str);
                for field in indexable_fields.iter() {
                  match instance.get_value(&field.try_into()?) {
                    Ok(result) => {
                      let value = if let Some(value) = result {
                        value.to_string()
                      } else {
                        "undefined".to_string()
                      };
                      // Fill the hash map with the requested field
                      data.insert(field.to_string(), value);
                    }
                    Err(e) => {
                      print!("\r\x1b[2K");
                      io::stdout().flush()?;
                      eprintln!("{}: {}", filepathstr, e.details);
                      error_count += 1;
                    }
                  }
                }
                // Provide the hash map to the index store
                if let Err(e) = index_store.write(&data) {
                  print!("\r\x1b[2K");
                  io::stdout().flush()?;
                  eprintln!("{}: {:?}", filepathstr, e);
                  error_count += 1;
                }
                if !opt.log_files {
                  // Fancy display
                  if let Some(study_instance_uid) = data.get("StudyInstanceUID") {
                    study_set.insert(study_instance_uid.clone());
                  }
                  if let Some(series_instance_uid) = data.get("SeriesInstanceUID") {
                    series_set.insert(series_instance_uid.clone());
                  }
                  if let Ok(Some(modality)) = instance.get_value(&Modality) {
                    modality_set.insert(modality.to_string().clone());
                  }
                  let wheel = "-\\|/";
                  let w = wheel.as_bytes()[count / 10 % 4] as char;
                  print!(
                    "{} [{}] files scanned with [{}] studies and [{}] series found and [{}] errors\r",
                    w,
                    count,
                    study_set.len(),
                    series_set.len(),
                    error_count
                  );
                  io::stdout().flush()?;
                }
              }
              _ => (),
            }
          }
          Err(e) => {
            print!("\r\x1b[2K");
            io::stdout().flush()?;
            eprintln!("{}: {}", filepathstr, e.details);
            error_count += 1;
          }
        }
      }
    }
  }
  if !opt.no_transaction {
    index_store.end_transaction()?;
  }

  println!("{} files scanned with {} studies and {} series found with following modalities {:?} and {} errors      ",
    count, study_set.len(), series_set.len(), modality_set, error_count);
  Ok(())
}

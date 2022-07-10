#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]

use std::collections::HashMap;
use std::io::BufReader;
use std::fs::File;
use std::path::Path;
use std::fs::metadata;
use std::path::PathBuf;
use std::error::Error;
use structopt::StructOpt;
use walkdir::WalkDir;
use std::io::{self, Write};
use serde::{Deserialize};

use rdicom::dicom_tags;
use rdicom::instance::Instance;
use rdicom::misc::is_dicom_file;

const ESC: char = 27u8 as char;

#[derive(Deserialize, Debug)]
struct Fields {
  studies: Vec<String>,
  series: Vec<String>,
  instances: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct Indexing {
  fields: Fields,
}

#[derive(Deserialize, Debug)]
struct Config {
  indexing: Indexing,
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

#[derive(Debug, StructOpt)]
struct Opt {
    #[structopt(short, long, parse(try_from_str = file_exists))]
    config: PathBuf,
    #[structopt(short, long, parse(try_from_str = path_is_folder))]
    input_path: PathBuf,
    #[structopt(long)]
    csv_output: Option<PathBuf>,
}

trait IndexStore {
  fn write(&mut self, data: &HashMap<String, String>) -> Result<(), Box<dyn Error>>;
}

struct CSVIndexStore<W: Write> {
  writer: W,
  fields: Vec<String>
}

impl<W: Write> CSVIndexStore<W> {
  fn new(mut writer: W, fields: Vec<String>) -> Self {
    let header = fields.iter().map(|s| String::from("\"") + s + "\"").collect::<Vec<String>>().join(",");
    writeln!(writer, "{},\"filepath\"", header).unwrap();
    CSVIndexStore { writer, fields }
  }
}

impl<W: Write> IndexStore for CSVIndexStore<W> {
  fn write(self: &mut Self, data: &HashMap<String, String>) -> Result<(), Box<dyn Error>> {
    for field in &self.fields {
      match write!(self.writer, "\"{}\",", data.get(field).unwrap_or(&"undefined".to_string())) {
        Ok(_) => (),
        Err(e) => return Err(Box::new(e)),
      }
    }
    writeln!(self.writer, "\"{}\"", data.get("filepath").unwrap_or(&"undefined".to_string()))?;
    Ok(())
  }
}

fn walk(input_path: &str, f: &mut dyn FnMut(&Path) -> ()) -> Result<(), Box<dyn Error>> {
  let mut count = 0;
  for result in WalkDir::new(input_path) {
    let entry = result?;
    let filepath = entry.path();
    let wheel = "-\\|/";
    if filepath.is_file() {
      f(filepath);
      let w = wheel.as_bytes()[count / 10 % 4] as char;
      eprint!("{}[2K\r{} {}", ESC, w, filepath.display());
      io::stdout().flush().unwrap();
      count += 1;
    }
  }
  eprintln!("{}[2K\r{} files found", ESC, count);
  Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
  // Retrieve options
  let opt = Opt::from_args();
  // Load the config
  let config_file = std::fs::read_to_string(&opt.config)?;
  let config: Config = serde_yaml::from_str(&config_file)?;
  // Create an vector of fields to write in the index
  let mut indexable_fields = config.indexing.fields.series.into_iter().chain(
    config.indexing.fields.studies.into_iter().chain(
      config.indexing.fields.instances.into_iter(),
    ),
  ).collect::<Vec<String>>();
  // Create an index store depending on the options
  let writer = if let Some(csv_output) = opt.csv_output {
    Box::new(File::create(csv_output)?) as Box<dyn Write>
  } else {
    Box::new(io::stdout()) as Box<dyn Write>
  };
  let mut index_store = CSVIndexStore::new(writer, indexable_fields.to_vec());
  // Walk all the files in the provided input folder
  let _ = walk(&opt.input_path.to_string_lossy(), &mut move |filepath: &Path| {
    // For each file, check it is a dicom file, load it and parse the requested fields
    if is_dicom_file(&filepath.to_string_lossy()) {
      match Instance::from_filepath(&filepath.to_string_lossy().to_string()) {
        Ok(instance) => {
          let mut data = HashMap::<String, String>::new();
          // We want the filepath in the index by default
          data.insert("filepath".to_string(), filepath.to_string_lossy().to_string());
          for field in indexable_fields.iter() {
            match instance.get_value(&field.into()) {
              Ok(result) => {
                let value = if let Some(value) = result { value.to_string() } else { "undefined".to_string() };
                // Fill the hash map with the requested field
                data.insert(field.to_string(), value);
              }
              Err(e) => eprintln!("{:?}", e),
            }
          }
          // Provide the hash map to the index store
          index_store.write(&data).unwrap();
        },
        Err(e) => eprintln!("{:?}", e),
      }
    }
  });
  Ok(())
}

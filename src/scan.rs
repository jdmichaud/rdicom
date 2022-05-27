#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]

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
}

fn walk(input_path: &str, f: &dyn Fn(&Path) -> ()) -> Result<(), Box<dyn Error>> {
  let mut count = 0;
  for result in WalkDir::new(input_path) {
    let entry = result?;
    let filepath = entry.path();
    let wheel = "-\\|/";
    if filepath.is_file() {
      f(filepath);
      let w = wheel.as_bytes()[count / 10 % 4] as char;
      // print!("{}[2K\r{} {}", ESC, w, filepath.display());
      io::stdout().flush().unwrap();
      count += 1;
    }
  }
  // println!("{}[2K\r{} files found", ESC, count);
  Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
  let opt = Opt::from_args();
  let config_file = std::fs::read_to_string(&opt.config)?;
  let config: Config = serde_yaml::from_str(&config_file)?;
  let _ = walk(&opt.input_path.to_string_lossy(), &|filepath: &Path| {
    if is_dicom_file(&filepath.to_string_lossy()) {
      let f = File::open(filepath).unwrap();
      match Instance::from_buf_reader(BufReader::new(f)) {
        Ok(instance) => {
          let indexable_fields = config.indexing.fields.series.iter().chain(
            config.indexing.fields.studies.iter().chain(
              config.indexing.fields.instances.iter(),
            ),
          );
          println!("{:?}", indexable_fields);
          for field in indexable_fields {
            if let Some(value) = instance.get_value(&field.into()) {
              println!("\"{}\",\"{}\",\"{}\"", field, value.to_string(), filepath.to_string_lossy());
            }
          }
        },
        Err(e) => eprintln!("{:?}", e),
      }
    }
  });
  Ok(())
}

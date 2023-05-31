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

#![allow(unused_variables)]
#![allow(dead_code)]

#[macro_use]
extern crate log;
extern crate simplelog;

use once_cell::sync::Lazy;
use rdicom::dicom_representation::{json2dcm, DicomAttributeJson};
use rdicom::error::DicomError;
use serde::ser::SerializeMap;
use serde::Serializer;
use serde::{de, Deserialize, Deserializer, Serialize};

use simplelog::{
  ColorChoice, CombinedLogger, Config, LevelFilter, SharedLogger, TermLogger, TerminalMode,
  WriteLogger,
};
use sqlite::Connection;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::convert::Infallible;
use std::convert::TryInto;
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{BufReader, BufWriter, Cursor, Read, Seek, Write};
use std::net::IpAddr;
use std::path::PathBuf;
use std::str::from_utf8;
use std::str::FromStr;
use structopt::clap::AppSettings;
use structopt::StructOpt;
use warp::http::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use warp::http::Response;
use warp::{reject, Filter, Rejection};

use crate::index_store::IndexStore;
use rdicom::dicom_tags;
use rdicom::instance::Instance;
use rdicom::tags::Tag;

mod config;
mod db;
mod index_store;

use index_store::SqlIndexStoreWithMutex;

// r"^/instances$",
// r"^/instances/(?P<SOPInstanceUID>[^/?#]*)$",
// r"^/instances/(?P<SOPInstanceUID>[^/?#]*)/frames/(?P<uid>[^/?#]*)$",
// r"^/instances/(?P<SOPInstanceUID>[^/?#]*)/rendered$",
// r"^/instances/(?P<SOPInstanceUID>[^/?#]*)/thumbnail$",
// r"^/instances/(?P<SOPInstanceUID>[^/?#]*)/(?P<tag>[^/?#]*)$",
// r"^/series$",
// r"^/series/(?P<SeriesInstanceUID>[^/?#]*)$",
// r"^/series/(?P<SeriesInstanceUID>[^/?#]*)/instances$",
// r"^/series/(?P<SeriesInstanceUID>[^/?#]*)/instances/(?P<SOPInstanceUID>[^/?#]*)$",
// r"^/series/(?P<SeriesInstanceUID>[^/?#]*)/instances/(?P<SOPInstanceUID>[^/?#]*)/frames/(?P<uid>[^/?#]*)$",
// r"^/series/(?P<SeriesInstanceUID>[^/?#]*)/instances/(?P<SOPInstanceUID>[^/?#]*)/rendered$",
// r"^/series/(?P<SeriesInstanceUID>[^/?#]*)/instances/(?P<SOPInstanceUID>[^/?#]*)/thumbnail$",
// r"^/series/(?P<SeriesInstanceUID>[^/?#]*)/rendered$",
// r"^/series/(?P<SeriesInstanceUID>[^/?#]*)/thumbnail$",
// r"^/studies$",
// r"^/studies/(?P<StudyInstanceUID>[^/?#]*)$",
// r"^/studies/(?P<StudyInstanceUID>[^/?#]*)/series$",
// r"^/studies/(?P<StudyInstanceUID>[^/?#]*)/series/(?P<SeriesInstanceUID>[^/?#]*)$",
// r"^/studies/(?P<StudyInstanceUID>[^/?#]*)/series/(?P<SeriesInstanceUID>[^/?#]*)/instances$",
// r"^/studies/(?P<StudyInstanceUID>[^/?#]*)/series/(?P<SeriesInstanceUID>[^/?#]*)/instances/(?P<SOPInstanceUID>[^/?#]*)$",
// r"^/studies/(?P<StudyInstanceUID>[^/?#]*)/series/(?P<SeriesInstanceUID>[^/?#]*)/instances/(?P<SOPInstanceUID>[^/?#]*)/frames/(?P<uid>[^/?#]*)$",
// r"^/studies/(?P<StudyInstanceUID>[^/?#]*)/series/(?P<SeriesInstanceUID>[^/?#]*)/instances/(?P<SOPInstanceUID>[^/?#]*)/rendered$",
// r"^/studies/(?P<StudyInstanceUID>[^/?#]*)/series/(?P<SeriesInstanceUID>[^/?#]*)/instances/(?P<SOPInstanceUID>[^/?#]*)/thumbnail$",
// r"^/studies/(?P<StudyInstanceUID>[^/?#]*)/series/(?P<SeriesInstanceUID>[^/?#]*)/rendered$",
// r"^/studies/(?P<StudyInstanceUID>[^/?#]*)/series/(?P<SeriesInstanceUID>[^/?#]*)/thumbnail$",
// r"^/studies/(?P<StudyInstanceUID>[^/?#]*)/thumbnail$"

fn file_exists(path: &str) -> Result<PathBuf, Box<dyn Error>> {
  let path_buf = PathBuf::from(path);
  if path_buf.exists() {
    Ok(path_buf)
  } else {
    Err(format!("{} does not exists", path).into())
  }
}

/// A simple DICOMWeb server
#[derive(Debug, StructOpt)]
#[structopt(
  name = format!("serve {} ({} {})", env!("GIT_HASH"), env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")),
  no_version,
  global_settings = &[AppSettings::DisableVersion]
)]
struct Opt {
  /// Port to listen to
  #[structopt(default_value = "8080", short, long)]
  port: u16,
  /// Host to serve
  #[structopt(default_value = "127.0.0.1", short = "o", long)]
  host: String,
  /// Sqlite database
  #[structopt(short, long)]
  sqlfile: String,
  /// Database config (necessary to create a database or add to the database)
  #[structopt(short, long, parse(try_from_str = file_exists))]
  config: Option<PathBuf>,
  // #[structopt(short="V", long)]
  // version: bool,
  /// DICOM files root path (root path provided to the scan tool)
  #[structopt(short = "d", long)]
  dcmpath: String,
  /// Add some logs on the console
  #[structopt(short, long)]
  verbose: bool,
  /// Log file path. No logs if not specified
  #[structopt(short, long)]
  logfile: Option<String>,
}

#[derive(Debug)]
struct NotAUniqueIdentifier;
impl reject::Reject for NotAUniqueIdentifier {}

#[derive(Debug)]
struct ApplicationError {
  message: String,
}
impl reject::Reject for ApplicationError {}

/// Extract a UI, or reject with NotAUniqueIdentifier.
fn unique_identifier() -> impl Filter<Extract = (String,), Error = Rejection> + Copy {
  warp::path::param().and_then(|ui: String| async {
    if ui.chars().all(|c| c.is_alphanumeric() || c == '.') {
      Ok(ui)
    } else {
      Err(reject::custom(NotAUniqueIdentifier))
    }
  })
}

struct MySerdeJsonError(serde_json::Error);

/**
 * Implements serialization of a serde_json error to be returned to the client.
 */
impl Serialize for MySerdeJsonError {
  fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    let mut map = serializer.serialize_map(Some(3))?;
    map.serialize_entry("line", &self.0.line())?;
    map.serialize_entry("column", &self.0.column())?;
    map.serialize_entry("error", &format!("{}", self.0))?;
    map.end()
  }
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum HttpErrorPayload {
  SerdeJsonErrorPayload {
    line: usize,
    column: usize,
    error: String,
  },
  SimpleErrorPayload {
    error: String,
  },
}

#[derive(Debug)]
struct HttpError {
  pub status: u16,
  pub payload: HttpErrorPayload,
}

impl HttpError {
  pub fn new(status: u16, msg: &str) -> HttpError {
    HttpError {
      status,
      payload: HttpErrorPayload::SimpleErrorPayload {
        error: msg.to_string(),
      },
    }
  }

  pub fn from_payload(status: u16, payload: HttpErrorPayload) -> HttpError {
    HttpError { status, payload }
  }

  pub fn from_json_error(status: u16, error: serde_json::Error) -> HttpError {
    HttpError {
      status,
      payload: HttpErrorPayload::SerdeJsonErrorPayload {
        line: error.line(),
        column: error.column(),
        error: format!("{}", error),
      },
    }
  }

  pub fn from_error(status: u16, error: &impl Error) -> HttpError {
    HttpError {
      status,
      payload: HttpErrorPayload::SimpleErrorPayload {
        error: error.to_string(),
      },
    }
  }
}

// Convert all the query parameters that can convert to a DICOM field (00200010=1.2.3.4.5) to
// an <Tag, String> entry in a HashMap.
fn query_param_to_search_terms(
) -> impl Filter<Extract = (HashMap<Tag, String>,), Error = Rejection> + Copy {
  warp::query::<HashMap<String, String>>().and_then(|q: HashMap<String, String>| async move {
    if true {
      Ok(
        q.into_iter()
          .filter_map(|(k, v)| {
            if let Ok(tag) = TryInto::<Tag>::try_into(&k) {
              Some((tag, v))
            } else {
              None
            }
          })
          // .map(|(k, v)| ((&k).try_into().unwrap(), v))
          .collect::<HashMap<Tag, String>>(),
      )
    } else {
      // TODO: Without the else clause, rust complains. Need to figure out why.
      Err(reject::custom(NotAUniqueIdentifier))
    }
  })
}

// For some reason, serde can't deserialize an array of String, so we provide a
// custom function that do so.
fn deserialize_array<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
  D: Deserializer<'de>,
{
  struct VectorStringVisitor;

  impl<'de> de::Visitor<'de> for VectorStringVisitor {
    type Value = Option<Vec<String>>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
      formatter.write_str("a vector of string")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
      E: de::Error,
    {
      Ok(Some(
        v.split(',').map(String::from).collect::<Vec<String>>(),
      ))
    }
  }

  deserializer.deserialize_any(VectorStringVisitor)
}

#[derive(Debug, Deserialize)]
struct QidoQueryParameters {
  limit: Option<usize>,
  offset: Option<usize>,
  fuzzymatching: Option<bool>,
  // Serde doesn't know how to deserialize an array
  #[serde(default)] // Allow the value to not be present in the url
  #[serde(deserialize_with = "deserialize_array")] // Help Serde to deserialize an array...
  includefield: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
enum AnnotationType {
  PATIENT,
  TECHNIQUE,
}

#[derive(Debug, Deserialize)]
struct WadoQueryParameters {
  annotation: Option<Vec<AnnotationType>>,
  quality: Option<f32>,
  viewport: Option<Vec<usize>>,
  window: Option<Vec<i32>>,
}

mod capabilities {

  use serde::{Deserialize, Serialize};

  /*
   * Below are the structures used to represent capabilities.
   */

  #[derive(Debug, Serialize, Deserialize, PartialEq)]
  struct Optn {
    value: String,
  }

  #[derive(Debug, Serialize, Deserialize, PartialEq)]
  struct Param {
    #[serde(rename = "@name")]
    name: String,
    #[serde(rename = "@style")]
    style: String,
    #[serde(rename = "@required")]
    required: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    default: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    href: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<Vec<Optn>>,
  }

  #[derive(Debug, Serialize, Deserialize, PartialEq)]
  struct Request {
    param: Vec<Param>,
  }

  #[derive(Debug, Serialize, Deserialize, PartialEq)]
  struct Representation {
    #[serde(rename = "@mediaType")]
    media_type: String,
  }

  #[derive(Debug, Serialize, Deserialize, PartialEq)]
  enum Status {
    #[serde(rename = "200")]
    OK,
    #[serde(rename = "202")]
    Accepted,
    #[serde(rename = "206")]
    PartialContent,
    #[serde(rename = "304")]
    NotModified,
    #[serde(rename = "400")]
    BadRequest,
    #[serde(rename = "409")]
    Conflict,
    #[serde(rename = "415")]
    UnsupportedMediaType,
    #[serde(rename = "501")]
    Unimplemented,
  }

  #[derive(Debug, Serialize, Deserialize, PartialEq)]
  struct Response {
    #[serde(rename = "@status")]
    status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    representation: Option<Representation>,
  }

  #[derive(Debug, Serialize, Deserialize)]
  struct Method {
    #[serde(rename = "@name")]
    name: String,
    #[serde(rename = "@id")]
    id: String,
    #[serde(rename = "request")]
    requests: Vec<Request>,
    #[serde(rename = "response")]
    responses: Vec<Response>,
  }

  #[derive(Debug, Serialize, Deserialize)]
  struct Resource {
    #[serde(rename = "@path")]
    path: String,
    #[serde(rename = "method")]
    methods: Vec<Method>,
  }

  #[derive(Debug, Serialize, Deserialize)]
  struct Resources {
    #[serde(rename = "@base")]
    base: String,
    resource: Vec<Resource>,
  }

  #[derive(Debug, Serialize, Deserialize)]
  pub struct Application {
    resources: Resources,
  }

  // Embed the capabilities description in the executable
  pub const CAPABILITIES_STR: &str = include_str!("capabilities.xml");
}

// Retrieves the column present in the index
fn get_indexed_fields(connection: &Connection) -> Result<Vec<String>, Box<dyn Error>> {
  Ok(
    connection
      .prepare("PRAGMA table_info(dicom_index);")?
      .into_iter()
      // TODO: get rid of this unwrap
      .map(|row| String::from(row.unwrap().read::<&str, _>(1)))
      .collect::<Vec<String>>(),
  )
}

fn map_to_entry(tag_map: &HashMap<String, String>) -> String {
  format!(
    "{{ {} }}",
    tag_map
      .iter()
      .map(|(key, value)| {
        // Try to convert the column name to a tag
        let tag_result: Result<Tag, DicomError> = key.try_into();
        match tag_result {
          Ok(tag) => format!(
            // We have a Dicom that we will format according to the DicomWeb standard
            // "00080030": {
            //   "vr": "TM",
            //   "Value": ["131600.0000"]
            // },
            "\"{:04x}{:04x}\": {{ \"vr\": \"{}\", \"Value\": [ \"{}\" ] }}",
            // TODO: The replace here is an ugly workaround which is probably going to cause more
            // problem than it will solve.
            tag.group,
            tag.element,
            tag.vr,
            value.replace("\\", ","),
          ),
          // Otherwise, just dump the key in the object
          _ => format!("\"{key}\": \"{value}\""),
        }
      })
      .collect::<Vec<String>>()
      .join(",")
  )
}

// Create an SQL where clause based on the search_term and query parameters.
fn create_where_clause(
  params: &QidoQueryParameters,
  search_terms: &HashMap<Tag, String>,
  indexed_fields: &[String],
) -> String {
  // limit
  // offset
  // fuzzymatching
  // includefield
  let fuzzymatching = params.fuzzymatching.unwrap_or(false);

  search_terms
    .iter()
    .filter(|(field, _)| indexed_fields.contains(&field.name.to_owned()))
    .fold(String::new(), |mut acc, (field, value)| {
      if acc.is_empty() {
        acc += "WHERE ";
      } else {
        acc += " AND ";
      }
      acc
        + &format!(
          "{}{}{}{}",
          field.name,
          if fuzzymatching { " LIKE '%" } else { "='" },
          value,
          if fuzzymatching { "%'" } else { "'" },
        )
    })
}

fn create_limit_clause(params: &QidoQueryParameters) -> String {
  let limit = params.limit.unwrap_or(u32::MAX as usize);
  let offset = params.offset.unwrap_or(0);

  format!("LIMIT {limit} OFFSET {offset}")
}

/**
 * The InstanceFactory trait abstract away the access to bulk data.
 * FSInstanceFactory implements the trait to access to file on a file system.
 * MemoryInstanceFactory implements the trait to access data from memory. This
 * allows the implementation of a ':memory:' option like sqlite provides.
 */

trait InstanceFactory<R: Read + Seek, W: Write> {
  fn get_reader(&self, path: &str) -> Result<R, DicomError>;
  fn get_writer(&self, path: &str) -> Result<W, DicomError>;
}

#[derive(Clone)]
struct FSInstanceFactory {
  dcmpath: String,
}

unsafe impl Sync for FSInstanceFactory {}
unsafe impl Send for FSInstanceFactory {}

impl FSInstanceFactory {
  fn new(dcmpath: &str) -> FSInstanceFactory {
    FSInstanceFactory {
      dcmpath: String::from(dcmpath),
    }
  }
}

impl InstanceFactory<BufReader<File>, BufWriter<File>> for FSInstanceFactory {
  fn get_reader(&self, path: &str) -> Result<BufReader<File>, DicomError> {
    let tmp = std::path::Path::new(&self.dcmpath).join(std::path::Path::new(path));
    let f = File::open(&*tmp.to_string_lossy())?;
    Ok(BufReader::new(f))
  }

  fn get_writer(&self, path: &str) -> Result<BufWriter<File>, DicomError> {
    let path = std::path::Path::new(&self.dcmpath).join(std::path::Path::new(path));
    if path.exists() {
      error!("{:?} file already exists, cannot overwrite file", path);
      Err(DicomError::new("File already exists"))
    } else {
      let f = File::create(&*path.to_string_lossy())?;
      Ok(BufWriter::new(f))
    }
  }
}

#[derive(Clone)]
struct MemoryInstanceFactory {
  files: HashMap<String, String>,
}

unsafe impl Sync for MemoryInstanceFactory {}
unsafe impl Send for MemoryInstanceFactory {}

impl MemoryInstanceFactory {
  fn new() -> MemoryInstanceFactory {
    MemoryInstanceFactory {
      files: HashMap::<String, String>::new(),
    }
  }
}

impl InstanceFactory<std::io::Cursor<Vec<u8>>, std::io::Cursor<Vec<u8>>> for MemoryInstanceFactory {
  fn get_reader(&self, path: &str) -> Result<std::io::Cursor<Vec<u8>>, DicomError> {
    println!("<- {}", path);
    Ok(Cursor::new(Vec::<u8>::new()))
  }
  fn get_writer(&self, path: &str) -> Result<std::io::Cursor<Vec<u8>>, DicomError> {
    println!("-> {}", path);
    Ok(Cursor::new(Vec::<u8>::new()))
  }
}

/**
 * Retrieve the fields from the index according to the search terms and enrich
 * the data from the index with the data from the DICOM files if necessary.
 */
fn get_entries<R: Read + Seek, W: Write, T: InstanceFactory<R, W>>(
  connection: &Connection,
  instance_factory: &T,
  params: &QidoQueryParameters,
  search_terms: &HashMap<Tag, String>,
  entry_type: &str,
) -> Result<Vec<HashMap<String, String>>, Box<dyn Error>> {
  let indexed_fields = get_indexed_fields(connection)?;
  // First retrieve the indexed fields present in the DB
  let query = &format!(
    "SELECT * FROM dicom_index {} group by {} {};",
    // Will restrict the data to what is being searched
    create_where_clause(params, search_terms, &indexed_fields),
    entry_type,
    create_limit_clause(params),
  );
  // println!("query: {}", query);
  let mut entries = db::query(connection, query)?;
  // println!("entries {:?}", entries);
  // Get the includefields not present in the index
  if let Some(includefield) = &params.includefield {
    let fields_to_fetch: Vec<String> = includefield
      .iter()
      .filter(|field| !indexed_fields.contains(field))
      .cloned()
      .collect::<_>();
    // println!("fields_to_fetch {:?}", fields_to_fetch);
    if !fields_to_fetch.is_empty() {
      for item in &mut entries {
        if let Some(rfilepath) = item.get("filepath") {
          let reader = instance_factory.get_reader(rfilepath)?;
          let instance = Instance::from_reader(reader)?;
          // Go through those missing fields from the index and enrich the data from the index
          for field in &fields_to_fetch {
            if let Some(field_value) = instance.get_value(&field.try_into()?)? {
              // TODO: Manage nested fields
              item.insert(field.to_string(), field_value.to_string());
            }
          }
        }
      }
    }
  }
  Ok(entries)
}

fn get_studies<R: Read + Seek, W: Write, T: InstanceFactory<R, W>>(
  connection: &Connection,
  instance_factory: &T,
  params: &QidoQueryParameters,
  search_terms: &HashMap<Tag, String>,
) -> Result<Vec<HashMap<String, String>>, Box<dyn Error>> {
  // select distinct StudyDate, StudyTime, AccessionNumber, ModalitiesInStudy, ReferringPhysicianName, PatientName, PatientID, StudyInstanceUID, StudyID from dicom_index group by StudyInstanceUID;
  get_entries(
    connection,
    instance_factory,
    params,
    search_terms,
    "StudyInstanceUID",
  )
}

fn get_series<R: Read + Seek, W: Write, T: InstanceFactory<R, W>>(
  connection: &Connection,
  instance_factory: &T,
  params: &QidoQueryParameters,
  search_terms: &HashMap<Tag, String>,
) -> Result<Vec<HashMap<String, String>>, Box<dyn Error>> {
  get_entries(
    connection,
    instance_factory,
    params,
    search_terms,
    "SeriesInstanceUID",
  )
}

fn get_instances<R: Read + Seek, W: Write, T: InstanceFactory<R, W>>(
  connection: &Connection,
  instance_factory: &T,
  params: &QidoQueryParameters,
  search_terms: &HashMap<Tag, String>,
) -> Result<Vec<HashMap<String, String>>, Box<dyn Error>> {
  get_entries(
    connection,
    instance_factory,
    params,
    search_terms,
    "filepath",
  )
}

fn get_study(ui: &str) -> HashMap<String, String> {
  HashMap::from([(String::from("link"), ui.to_string())])
}

fn get_serie(ui: &str) -> HashMap<String, String> {
  HashMap::from([(String::from("link"), ui.to_string())])
}

fn get_instance(ui: &str) -> HashMap<String, String> {
  HashMap::from([(String::from("link"), ui.to_string())])
}

fn generate_json_response(data: &[HashMap<String, String>]) -> String {
  format!(
    "[{}]",
    data
      .iter()
      .map(map_to_entry)
      .collect::<Vec<String>>()
      .join(",")
  )
}

fn with_db<'a>(
  sqlfile: &str,
) -> impl Filter<Extract = (Connection,), Error = Infallible> + Clone + 'a {
  let sqlpath = PathBuf::from_str(sqlfile).unwrap();
  warp::any().map(move || Connection::open(&sqlpath).unwrap())
}

fn with_instance_factory<R: Read + Seek, W: Write, T: InstanceFactory<R, W> + Clone + Send>(
  instance_factory: T,
) -> impl Filter<Extract = (T,), Error = Infallible> + Clone {
  warp::any().map(move || instance_factory.clone())
}

fn with_index_store(
  index_store: SqlIndexStoreWithMutex,
) -> impl Filter<Extract = (SqlIndexStoreWithMutex,), Error = Infallible> + Clone {
  warp::any().map(move || index_store.clone())
}

fn dicom_attribute_json_to_string(attribute: &DicomAttributeJson) -> String {
  String::try_from(attribute.payload.clone().unwrap()).unwrap_or("undefined".to_string())
}

fn do_store<R: Read + Seek, W: Write, T: InstanceFactory<R, W> + Clone + Send>(
  connection: Connection,
  instance_factory: T,
  index_store: &mut SqlIndexStoreWithMutex,
  accept_header: &HeaderValue,
  body: &warp::hyper::body::Bytes,
) -> Result<(), HttpError> {
  let accept_header = get_accept_headers(accept_header)
    .map_err(|e| HttpError::new(500, "to_str failed in get_accept_headers"))?;
  let accept = get_accept_format(
    &accept_header,
    &["application/dicom+json", "application/json"],
  )
  .map_err(|e| HttpError::new(406, &e.details))?;
  match accept.format.as_str() {
    "application/dicom+json" | "application/json" => {
      let body: String = from_utf8(body.to_vec().as_slice())
        .map(str::to_string)
        .map_err(|e| {
          HttpError::new(
            400,
            &format!("utf-8 error. Message valid up to {} byte", e.valid_up_to()),
          )
        })?;
      let dataset = serde_json::from_str::<BTreeMap<String, DicomAttributeJson>>(&body)
        .map_err(|e| HttpError::from_json_error(400, e))?;
      let sop_instance_uid = String::try_from(
        dataset
          .get(&dicom_tags::SOPInstanceUID.to_string())
          .unwrap()
          .payload
          .clone()
          .unwrap(),
      )
      .map_err(|e| HttpError::new(400, &e.details))?;
      let filename = &format!("{}.dcm", sop_instance_uid);
      let mut writer = instance_factory.get_writer(filename).map_err(|e| {
        warn!("Could not get a writer for {}: {}", filename, e);
        HttpError::new(500, &e.details)
      })?;
      // Write the file
      json2dcm::json2dcm(&mut writer, &dataset).map_err(|e| {
        warn!(
          "Error while streaming the dicom file to {}: {}",
          filename, e
        );
        HttpError::new(500, &e.details)
      })?;
      // Update the index
      let mut data: HashMap<String, String> = dataset
        .iter()
        .map(|(k, v)| {
          (
            Tag::try_from(k).unwrap().name.to_string(),
            dicom_attribute_json_to_string(v),
          )
        })
        .collect();
      data.insert("filepath".to_string(), filename.to_string());
      index_store.write(&data).map_err(|e| {
        warn!(
          "Error while updating the index file to the DB for {}: {}",
          filename, e
        );
        HttpError::new(500, &format!("{}", e))
      })?;
      Ok(())
    }
    _ => Err(HttpError::new(406, "Unhandled Content-Type")),
  }
}

#[derive(Debug)]
struct RDicomRejection {
  pub details: String,
  pub status: warp::http::StatusCode,
  pub accept_header: HeaderValue,
}

impl reject::Reject for RDicomRejection {}

fn post_store_api<R: Read + Seek, W: Write, T: InstanceFactory<R, W> + Clone + Send>(
  sqlfile: &str,
  instance_factory: T,
  index_store: SqlIndexStoreWithMutex,
) -> impl Filter<Extract = (impl warp::Reply,), Error = Rejection> + Clone {
  // POST  ../studies  Store instances.
  let store_instances = warp::path("studies")
    .and(warp::path::end())
    .and(warp::filters::header::value("Content-Type"))
    .and(warp::filters::body::bytes())
    .and(with_db(sqlfile))
    .and(with_instance_factory(instance_factory))
    .and(with_index_store(index_store))
    // .and(warp::body::content_length_limit(1024 * 1000)) // 1G
    .map(
      |accept_header: HeaderValue,
       body: warp::hyper::body::Bytes,
       connection: Connection,
       instance_factory: T,
       mut index_store: SqlIndexStoreWithMutex| {
        // Is it single part or multipart?
        let multipart = accept_header == "multipart/related";
        if multipart {
          Response::builder()
            .status(warp::http::StatusCode::NOT_IMPLEMENTED)
            .body("".to_string())
        } else {
          // TODO: get rid if this unwrap
          if let Err(e) = do_store(
            connection,
            instance_factory,
            &mut index_store,
            &accept_header,
            &body,
          ) {
            return Response::builder()
              .status(e.status)
              .header(warp::http::header::CONTENT_ENCODING, "application/json")
              .body(serde_json::to_string(&e.payload).unwrap());
          }
          Response::builder()
            .status(warp::http::StatusCode::OK)
            .body("".to_string())
        }
      },
    );
  // POST  ../studies/{study}  Store instances for a specific study.
  let store_instances_in_study = warp::path("studies")
    .and(unique_identifier())
    .and(warp::path::end())
    .map(|study_uid: String| warp::reply::with_status("", warp::http::StatusCode::NOT_IMPLEMENTED));

  warp::post()
    .and(store_instances)
    .or(store_instances_in_study)
}

/**
 * https://www.dicomstandard.org/using/dicomweb/query-qido-rs/
 */
fn get_query_api<R: Read + Seek, W: Write, T: InstanceFactory<R, W> + Clone + Send>(
  sqlfile: &str,
  instance_factory: T,
) -> impl Filter<Extract = (impl warp::Reply,), Error = Rejection> + Clone {
  // No literal constructor for HeaderMap, so have to allocate them here...
  let mut json_headers = HeaderMap::new();
  json_headers.insert(
    CONTENT_TYPE,
    "application/dicom+json; charset=utf-8".parse().unwrap(),
  );

  // GET {s}/studies?... Query for all the studies
  let studies = warp::path("studies")
    .and(warp::path::end())
    .and(warp::query::<QidoQueryParameters>())
    .and(query_param_to_search_terms())
    .and(with_db(sqlfile))
    // So many clones...
    .and(with_instance_factory(instance_factory.clone()))
    .and_then(
      |qido_params: QidoQueryParameters,
       search_terms: HashMap<Tag, String>,
       connection: Connection,
       instance_factory: T| async move {
        match get_studies(&connection, &instance_factory, &qido_params, &search_terms) {
          // Have to specify the type annotation here, see: https://stackoverflow.com/a/67413956/2603925
          Ok(studies) => Ok::<_, warp::Rejection>(generate_json_response(&studies)),
          // TODO: Can't use ? in the and_then handler because we can convert automatically from
          // DicomError to Reject. See: https://stackoverflow.com/a/65175925/2603925
          Err(e) => Err(warp::reject::custom(ApplicationError {
            message: e.to_string(),
          })),
        }
      },
    )
    .with(warp::reply::with::headers(json_headers.clone()));

  // GET {s}/series?... Query for all the series
  let series = warp::path("series")
    .and(warp::path::end())
    .and(warp::query::<QidoQueryParameters>())
    .and(query_param_to_search_terms())
    .and(with_db(sqlfile))
    .and(with_instance_factory(instance_factory.clone()))
    .and_then(
      |qido_params: QidoQueryParameters,
       search_terms: HashMap<Tag, String>,
       connection: Connection,
       instance_factory: T| async move {
        match get_series(&connection, &instance_factory, &qido_params, &search_terms) {
          Ok(series) => Ok::<_, warp::Rejection>(generate_json_response(&series)),
          Err(e) => Err(warp::reject::custom(ApplicationError {
            message: e.to_string(),
          })),
        }
      },
    )
    .with(warp::reply::with::headers(json_headers.clone()));

  // GET {s}/instances?... Query for all the instances
  let instances = warp::path("instances")
    .and(warp::path::end())
    .and(warp::query::<QidoQueryParameters>())
    .and(query_param_to_search_terms())
    .and(with_db(sqlfile))
    .and(with_instance_factory(instance_factory.clone()))
    .and_then(
      |qido_params: QidoQueryParameters,
       search_terms: HashMap<Tag, String>,
       connection: Connection,
       instance_factory: T| async move {
        match get_instances(&connection, &instance_factory, &qido_params, &search_terms) {
          Ok(instances) => Ok::<_, warp::Rejection>(generate_json_response(&instances)),
          Err(e) => Err(warp::reject::custom(ApplicationError {
            message: e.to_string(),
          })),
        }
      },
    )
    .with(warp::reply::with::headers(json_headers.clone()));

  // GET {s}/studies/{study}/series?...  Query for series in a study
  let studies_series = warp::path("studies")
    .and(unique_identifier())
    .and(warp::path("series"))
    .and(warp::path::end())
    .and(warp::query::<QidoQueryParameters>())
    .and(query_param_to_search_terms())
    .and(with_db(sqlfile))
    .and(with_instance_factory(instance_factory.clone()))
    .and_then(
      |study_uid: String,
       qido_params: QidoQueryParameters,
       mut search_terms: HashMap<Tag, String>,
       connection: Connection,
       instance_factory: T| async move {
        search_terms.insert(Tag::try_from("StudyInstanceUID").unwrap(), study_uid);
        match get_studies(&connection, &instance_factory, &qido_params, &search_terms) {
          Ok(studies) => Ok::<_, warp::Rejection>(generate_json_response(&studies)),
          Err(e) => Err(warp::reject::custom(ApplicationError {
            message: e.to_string(),
          })),
        }
      },
    )
    .with(warp::reply::with::headers(json_headers.clone()));

  // GET {s}/studies/{study}/series/{series}/instances?... Query for instances in a series
  let studies_series_instances = warp::path("studies")
    .and(unique_identifier())
    .and(warp::path("series"))
    .and(unique_identifier())
    .and(warp::path("instances"))
    .and(warp::path::end())
    .and(warp::query::<QidoQueryParameters>())
    .and(query_param_to_search_terms())
    .and(with_db(sqlfile))
    .and(with_instance_factory(instance_factory))
    .and_then(
      |study_uid: String,
       series_uid: String,
       qido_params: QidoQueryParameters,
       mut search_terms: HashMap<Tag, String>,
       connection: Connection,
       instance_factory: T| async move {
        search_terms.insert(Tag::try_from("StudyInstanceUID").unwrap(), study_uid);
        search_terms.insert(Tag::try_from("SeriesInstanceUID").unwrap(), series_uid);
        match get_studies(&connection, &instance_factory, &qido_params, &search_terms) {
          Ok(studies) => Ok::<_, warp::Rejection>(generate_json_response(&studies)),
          Err(e) => Err(warp::reject::custom(ApplicationError {
            message: e.to_string(),
          })),
        }
      },
    )
    .with(warp::reply::with::headers(json_headers.clone()));

  warp::get()
    .and(studies)
    .or(series)
    .or(instances)
    .or(studies_series)
    .or(studies_series_instances)
}

/**
 * https://www.dicomstandard.org/using/dicomweb/retrieve-wado-rs-and-wado-uri/
 */
fn get_retrieve_api(
  sqlfile: &str,
) -> impl Filter<Extract = (impl warp::Reply,), Error = Rejection> + Clone {
  // GET {s}/studies/{study} Retrieve entire study
  let studies = warp::path("studies")
    .and(unique_identifier())
    .and(warp::filters::header::value("accept"))
    .and(warp::path::end())
    // TODO: Find a way to return Not Implemented if the accept header is not application/json
    // .and_then(|study_uid: String, accept_header: HeaderValue| async move {
    //   if accept_header != "application/json" && accept_header != "application/dicom+json" {
    //     let message = String::from("Only 'application/json' or 'application/dicom+json' accept header are supported");
    //     Ok(warp::reply::with_status(message, warp::http::StatusCode::NOT_IMPLEMENTED))
    //   } else {
    //     Err(warp::reject::reject())
    //   }
    // });
    .and_then(|study_uid: String, accept_header: HeaderValue| async move {
      Ok::<_, warp::Rejection>(serde_json::to_string("").unwrap())
    });
  // GET {s}/studies/{study}/rendered  Retrieve rendered study
  let studies_rendered = warp::path("studies")
    .and(unique_identifier())
    .and(warp::path("rendered"))
    .and(warp::query::<WadoQueryParameters>())
    .and(warp::path::end())
    .map(|study_uid: String, params: WadoQueryParameters| {
      warp::reply::with_status("", warp::http::StatusCode::NOT_IMPLEMENTED)
    });
  // GET {s}/studies/{study}/series/{series} Retrieve entire series
  let studies_series = warp::path("studies")
    .and(unique_identifier())
    .and(warp::path("series"))
    .and(unique_identifier())
    .and(warp::path::end())
    .map(|study_uid: String, series_uid: String| {
      warp::reply::with_status("", warp::http::StatusCode::NOT_IMPLEMENTED)
    });
  // GET {s}/studies/{study}/series/{series}/rendered  Retrieve rendered series
  let studies_series_rendered = warp::path("studies")
    .and(unique_identifier())
    .and(warp::path("series"))
    .and(unique_identifier())
    .and(warp::path("rendered"))
    .and(warp::query::<WadoQueryParameters>())
    .and(warp::path::end())
    .map(
      |study_uid: String, series_uid: String, params: WadoQueryParameters| {
        warp::reply::with_status("", warp::http::StatusCode::NOT_IMPLEMENTED)
      },
    );
  // GET {s}/studies/{study}/series/{series}/metadata  Retrieve series metadata
  let studies_series_metadata = warp::path("studies")
    .and(unique_identifier())
    .and(warp::path("series"))
    .and(unique_identifier())
    .and(warp::path("metadata"))
    .and(warp::path::end())
    .map(|study_uid: String, series_uid: String| {
      warp::reply::with_status("", warp::http::StatusCode::NOT_IMPLEMENTED)
    });

  let series = warp::path("series")
    .and(unique_identifier())
    .and(warp::path::end())
    .map(|study_uid: String| warp::reply::with_status("", warp::http::StatusCode::NOT_IMPLEMENTED));
  let series_rendered = warp::path("series")
    .and(unique_identifier())
    .and(warp::path("rendered"))
    .and(warp::query::<WadoQueryParameters>())
    .and(warp::path::end())
    .map(|study_uid: String, params: WadoQueryParameters| {
      warp::reply::with_status("", warp::http::StatusCode::NOT_IMPLEMENTED)
    });

  // GET {s}/studies/{study}/series/{series}/instances/{instance}  Retrieve instance
  let studies_series_instances = warp::path("studies")
    .and(unique_identifier())
    .and(warp::path("series"))
    .and(unique_identifier())
    .and(warp::path("instances"))
    .and(unique_identifier())
    .and(warp::path::end())
    .map(
      |study_uid: String, series_uid: String, instance_uid: String| {
        warp::reply::with_status("", warp::http::StatusCode::NOT_IMPLEMENTED)
      },
    );
  // GET {s}/studies/{study}/series/{series}/instances/{instance}/rendered Retrieve rendered instance
  let studies_series_instances_rendered = warp::path("studies")
    .and(unique_identifier())
    .and(warp::path("series"))
    .and(unique_identifier())
    .and(warp::path("instances"))
    .and(unique_identifier())
    .and(warp::path("rendered"))
    .and(warp::query::<WadoQueryParameters>())
    .and(warp::path::end())
    .map(
      |study_uid: String, series_uid: String, instance_uid: String, params: WadoQueryParameters| {
        warp::reply::with_status("", warp::http::StatusCode::NOT_IMPLEMENTED)
      },
    );
  // GET {s}/studies/{study}/series/{series}/instances/{instance}/metadata Retrieve instance metadata
  let studies_series_instances_metadata = warp::path("studies")
    .and(unique_identifier())
    .and(warp::path("series"))
    .and(unique_identifier())
    .and(warp::path("instances"))
    .and(unique_identifier())
    .and(warp::path("metadata"))
    .and(warp::path::end())
    .map(
      |study_uid: String, series_uid: String, instance_uid: String| {
        warp::reply::with_status("", warp::http::StatusCode::NOT_IMPLEMENTED)
      },
    );

  let instance = warp::path("instances")
    .and(unique_identifier())
    .and(warp::path::end())
    .map(|instance_uid: String| {
      warp::reply::with_status("", warp::http::StatusCode::NOT_IMPLEMENTED)
    });

  let instance_rendered = warp::path("instances")
    .and(unique_identifier())
    .and(warp::path("rendered"))
    .and(warp::query::<WadoQueryParameters>())
    .and(warp::path::end())
    .map(|instance_uid: String, params: WadoQueryParameters| {
      warp::reply::with_status("", warp::http::StatusCode::NOT_IMPLEMENTED)
    });

  // GET {s}/studies/{study}/series/{series}/instances/{instance}/frames/{frames}  Retrieve frames in an instance
  let studies_series_instances = warp::path("studies")
    .and(unique_identifier())
    .and(warp::path("series"))
    .and(unique_identifier())
    .and(warp::path("instances"))
    .and(unique_identifier())
    .and(warp::path::end())
    .map(
      |study_uid: String, series_uid: String, instance_uid: String| {
        warp::reply::with_status("", warp::http::StatusCode::NOT_IMPLEMENTED)
      },
    );

  // GET {s}/{bulkdataURIReference}

  warp::get()
    .and(studies)
    .or(studies_rendered)
    .or(studies_series)
    .or(studies_series_rendered)
    .or(studies_series_metadata)
    .or(series)
    .or(series_rendered)
    .or(studies_series_instances)
    .or(studies_series_instances_rendered)
    .or(studies_series_instances_metadata)
    .or(instance)
    .or(instance_rendered)
}

fn get_delete_api(
  sqlfile: &str,
) -> impl Filter<Extract = (impl warp::Reply,), Error = Rejection> + Clone {
  // DELETE   ../studies/{study}  Delete all instances for a specific study.
  let delete_all_instances_from_study = warp::path("studies")
    .and(unique_identifier())
    .and(warp::path::end())
    .map(|study_uid: String| warp::reply::with_status("", warp::http::StatusCode::NOT_IMPLEMENTED));
  // DELETE   ../studies/{study}/series/{series}  Delete all instances for a specific series within a study.
  let delete_all_instance_from_series = warp::path("studies")
    .and(unique_identifier())
    .and(warp::path("series"))
    .and(unique_identifier())
    .and(warp::path::end())
    .map(|study_uid: String, series_uid: String| {
      warp::reply::with_status("", warp::http::StatusCode::NOT_IMPLEMENTED)
    });
  // DELETE  ../studies/{study}/series/{series}/instances/{instance}   Delete a specific instance within a series.
  let delete_instance = warp::path("studies")
    .and(unique_identifier())
    .and(warp::path("series"))
    .and(unique_identifier())
    .and(warp::path("instances"))
    .and(unique_identifier())
    .and(warp::path::end())
    .map(
      |study_uid: String, series_uid: String, instance_uid: String| {
        warp::reply::with_status("", warp::http::StatusCode::NOT_IMPLEMENTED)
      },
    );

  warp::delete()
    .and(delete_all_instances_from_study)
    .or(delete_all_instance_from_series)
    .or(delete_instance)
}

// This function receives a `Rejection` and tries to return a custom
// value, otherwise simply passes the rejection along.
async fn handle_rejection(err: Rejection) -> Result<impl warp::Reply, Infallible> {
  let code;
  let message: String;

  if err.is_not_found() {
    code = warp::http::StatusCode::NOT_FOUND;
    message = String::from("not found");
  } else if let Some(invalid_parameter) = err.find::<ApplicationError>() {
    code = warp::http::StatusCode::BAD_REQUEST;
    message = invalid_parameter.message.clone();
  } else if err.find::<NotAUniqueIdentifier>().is_some() {
    code = warp::http::StatusCode::BAD_REQUEST;
    message = String::from("path parameter is not a DICOM unique identifier");
  } else if err.find::<warp::reject::MethodNotAllowed>().is_some() {
    // We can handle a specific error, here METHOD_NOT_ALLOWED,
    // and render it however we want
    code = warp::http::StatusCode::METHOD_NOT_ALLOWED;
    message = String::from("method not allowed");
  } else {
    // We should have expected this... Just log and say its a 500
    code = warp::http::StatusCode::INTERNAL_SERVER_ERROR;
    message = format!("unhandled rejection: {:?}", err);
  }

  error!("{:?}", err);
  Ok(warp::reply::with_status(message, code))
}

// If configuration was provided, we check the database respects the config. If
// the database does not exists we create it with respect to the provided
// config. If no configuration was provided, we check the database exists with a
// 'dicom_index' table.
fn check_db(opt: &Opt) -> Result<(String, Vec<String>, Connection), Box<dyn Error>> {
  let connection = Connection::open(&opt.sqlfile)?;
  return match &opt.config {
    Some(filepath) => {
      // Load the config
      let config_file = std::fs::read_to_string(filepath)?;
      let config: config::Config = serde_yaml::from_str(&config_file)?;
      let mut indexable_fields = config
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
      indexable_fields.push("filepath".to_string());
      if db::query(
        &connection,
        &format!(
          "SELECT name FROM sqlite_master WHERE type='table' AND name='{}';",
          config.table_name
        ),
      )?
      .is_empty()
      {
        // We will create the requested table with the appropriate fields
        let table = indexable_fields
          .iter()
          .map(|s| s.to_string() + " TEXT NON NULL")
          .collect::<Vec<String>>()
          .join(",");

        let create_index_table_request = &format!(
          "CREATE TABLE IF NOT EXISTS {} ({});",
          config.table_name, table
        );
        connection.execute(create_index_table_request)?;
      }
      Ok((config.table_name, indexable_fields, connection))
    }
    None => {
      // Check the database exists and that the dicom_index table also exists.
      // If not, we need the config to tell us how to create that table.
      return if db::query(
        &connection,
        &format!(
          "SELECT name FROM sqlite_master WHERE type='table' AND name='{}';",
          "dicom_index"
        ),
      )?
      .is_empty()
      {
        Err(
          format!(
            "{} table does not exist in provided database. \
          To create a database from scratch you must provide a configuration file (--config)",
            "dicom_index"
          )
          .into(),
        )
      } else {
        Ok((
          "dicom_index".to_string(),
          get_indexed_fields(&connection)?,
          connection,
        ))
      };
    }
  };
}

// A convenient representation of the content of the accept header
#[derive(Debug)]
struct AcceptHeader {
  format: String,
  parameters: HashMap<String, String>,
}

/**
 * Returns the first entry in the request accept header that is available on the
 * server side.
 */
fn get_accept_format<'a>(
  accepts: &'a Vec<AcceptHeader>,
  availables: &'a [&'a str],
) -> Result<&'a AcceptHeader, DicomError> {
  for accept in accepts {
    if availables.contains(&accept.format.as_str()) {
      return Ok(accept);
    }
  }
  Err(DicomError::new(&format!(
    "Unsupported accept header {:?}, only {:?} accept header are supported",
    accepts, availables
  )))
}

/**
 * Convert the Accept/Content-Type header to something like:
 * [ format: application/json, parameters: { boundary: '---abcd1234---' }, ... ]
 */
fn get_accept_headers(accept_header: &HeaderValue) -> Result<Vec<AcceptHeader>, Box<dyn Error>> {
  Ok(
    accept_header
      .to_str()?
      .split(',')
      .map(|entry| {
        let mut subentries = entry.split(';');
        let format = subentries.next().unwrap();
        let mut parameters = HashMap::<String, String>::new();
        for subentry in subentries {
          let parameter = subentry
            .split('=')
            .map(String::from)
            .collect::<Vec<String>>();
          parameters.insert(parameter[0].clone(), parameter[1].clone());
        }
        AcceptHeader {
          format: String::from(format),
          parameters,
        }
      })
      .collect::<Vec<AcceptHeader>>(),
  )
}

fn capabilities(
  application: &'static capabilities::Application,
) -> impl Filter<Extract = (impl warp::Reply,), Error = Rejection> + Clone {
  let handlers = warp::filters::header::value("accept").map(move |accept_header: HeaderValue| {
    let accept_header = get_accept_headers(&accept_header).unwrap();
    match get_accept_format(
      &accept_header,
      &[
        "",
        "*/*",
        "application/vnd.sun.wadl+xml",
        "application/xml",
        "application/dicom+xml",
        "application/dicom+json",
        "application/json",
      ],
    ) {
      Ok(accept) => match accept.format.as_str() {
        "application/vnd.sun.wadl+xml"
        | "application/dicom+xml"
        | "application/xml"
        | ""
        | "*/*" => Response::builder()
          .header(warp::http::header::CONTENT_ENCODING, &accept.format)
          .status(warp::http::StatusCode::OK)
          .body(quick_xml::se::to_string(&application).unwrap()),
        "application/dicom+json" | "application/json" => Response::builder()
          .header(warp::http::header::CONTENT_ENCODING, &accept.format)
          .status(warp::http::StatusCode::OK)
          // Because of https://github.com/tafia/quick-xml/issues/582, json output
          // is polluted with field names starting with "@". We replace them here.
          // TODO: Write a intermediary serializer to handle these.
          .body(
            serde_json::to_string(&application)
              .unwrap()
              .replace('@', ""),
          ),
        _ => unreachable!(),
      },
      Err(e) => {
        warn!("Accept header format {:?} not accepted", accept_header);
        Response::builder()
          .status(warp::http::StatusCode::NOT_IMPLEMENTED)
          .body(e.details)
      }
    }
  });

  // Fresh for a week.
  let cache_header =
    warp::reply::with::default_header(warp::http::header::CACHE_CONTROL, "max-age=604800");

  let option = warp::path::end()
    // OPTIONS /
    .and(warp::options().and(handlers))
    .with(cache_header.clone());
  // GET /
  let get = warp::path::end()
    .and(warp::get().and(handlers))
    .with(cache_header);

  option.or(get)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  // Retrieve options
  let opt = Opt::from_args();

  let mut logconfig: Vec<Box<(dyn SharedLogger + 'static)>> = vec![];
  if opt.verbose {
    logconfig.push(TermLogger::new(
      LevelFilter::Warn,
      Config::default(),
      TerminalMode::Mixed,
      ColorChoice::Auto,
    ));
  }
  if let Some(logfile) = opt.logfile.clone() {
    logconfig.push(WriteLogger::new(
      LevelFilter::Trace,
      Config::default(),
      File::create(logfile)?,
    ));
  }
  CombinedLogger::init(logconfig)?;

  // Check the the status of the database and the option are coherent.
  let (table_name, indexable_fields, connection) = check_db(&opt)?;

  let log = warp::log::custom(|info| {
    eprintln!(
      "{} {} => {} (in {:?})",
      info.method(),
      info.path(),
      info.status(),
      info.elapsed()
    );
  });

  let server_header: &'static str = concat!("rdicomweb/", env!("CARGO_PKG_VERSION"));
  let mut headers = HeaderMap::new();
  headers.insert("server", HeaderValue::from_static(server_header));

  static APPLICATION: Lazy<capabilities::Application> =
    Lazy::new(|| quick_xml::de::from_str(capabilities::CAPABILITIES_STR).unwrap());

  // GET /
  let root = warp::get()
    .and(warp::path("about"))
    .map(move || server_header);

  let index_store = SqlIndexStoreWithMutex::new(connection, &table_name, indexable_fields)?;
  let routes = if opt.dcmpath == ":memory:" {
    let instance_factory = MemoryInstanceFactory::new();
    root
      .or(post_store_api(
        &opt.sqlfile,
        instance_factory.clone(),
        index_store,
      ))
      .or(get_query_api(&opt.sqlfile, instance_factory))
      .or(get_retrieve_api(&opt.sqlfile))
      .or(get_delete_api(&opt.sqlfile))
      .or(capabilities(&APPLICATION))
      .with(warp::cors().allow_any_origin())
      .with(warp::reply::with::headers(headers))
      .with(log)
      .recover(handle_rejection)
      .boxed()
  } else {
    root
      .or(post_store_api(
        &opt.sqlfile,
        FSInstanceFactory::new(&opt.dcmpath),
        index_store,
      ))
      .or(get_query_api(
        &opt.sqlfile,
        FSInstanceFactory::new(&opt.dcmpath),
      ))
      .or(get_retrieve_api(&opt.sqlfile))
      .or(get_delete_api(&opt.sqlfile))
      .or(capabilities(&APPLICATION))
      .with(warp::cors().allow_any_origin())
      .with(warp::reply::with::headers(headers))
      .with(log)
      .recover(handle_rejection)
      .boxed()
  };

  let host = opt.host;
  println!(
    "Serving HTTP on {} port {} (http://{}:{}/) with database {} ...",
    host, opt.port, host, opt.port, opt.sqlfile
  );
  info!(
    "Serving HTTP on {} port {} (http://{}:{}/) with database {} ...",
    host, opt.port, host, opt.port, opt.sqlfile
  );
  warp::serve(routes)
    .run((IpAddr::from_str(&host)?, opt.port))
    .await;

  Ok(())
}

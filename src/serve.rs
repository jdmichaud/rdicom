// Copyright (c) 2023-2025 Jean-Daniel Michaud
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

// Run serve for testing purposes (sqlite in memory and datapath in /tmp)
// cargo run --bin serve --features tools --target x86_64-unknown-linux-musl --\
//   --sqlfile :memory: --config config.yaml --dcmpath /tmp/ --verbose

#![allow(unused_variables)]
#![allow(dead_code)]

#[macro_use]
extern crate log;
extern crate simplelog;

use axum::{
  body::{Body, Bytes},
  extract::{rejection::JsonRejection, Path, Request},
  http::{header::ACCEPT, HeaderMap, StatusCode},
  middleware::{self, Next},
  response::{IntoResponse, Response},
  routing::{delete, get, options, post},
  Json, Router,
};
use axum_extra::extract::WithRejection;
use clap::Parser;
use http_body_util::BodyExt;
use once_cell::sync::Lazy;
use serde::ser::SerializeMap;
use serde::Serializer;
use serde::{de, Deserialize, Deserializer, Serialize};
use sqlite::{Connection, ConnectionThreadSafe};
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::convert::TryInto;
use std::env;
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{BufReader, BufWriter, Cursor, Read, Seek, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use tower_http::trace::{self, TraceLayer};
use tracing::Level;

use crate::dicom_representation::{json2dcm, DicomAttributeJson};
use crate::index_store::IndexStore;
use index_store::SqlIndexStoreWithMutex;
use rdicom::config_file::{self, ConfigProvenance};
use rdicom::dicom_tags;
use rdicom::error::DicomError;
use rdicom::instance::{DicomValue, Instance};
use rdicom::tags::Tag;

mod config;
mod db;
mod dicom_representation;
mod index_store;

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

// pub const CAPABILITIES_STR: &str = include_str!("capabilities.xml");
pub const SERVER_HEADER: &str = concat!("rdicomweb/", env!("CARGO_PKG_VERSION"));

const DEFAULT_CONFIG: &str = include_str!("../config.yaml");

/// A simple DICOMWeb server
#[derive(Debug, Parser)]
#[structopt(
  name = format!("serve {} ({} {})", env!("GIT_HASH"), env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")),
  version = "",
)]
struct Opt {
  /// Port to listen to
  #[arg(default_value = "8080", short, long)]
  port: u16,
  /// Host to serve
  #[arg(default_value = "127.0.0.1", short = 'o', long)]
  host: String,
  /// Sqlite database
  #[arg(short, long)]
  sqlfile: String,
  /// Database config (necessary to create a database or add to the database)
  #[arg(short, long)]
  config: Option<PathBuf>,
  /// Print the used configuration and exit.
  /// You can use this option to initialize the default config file with:
  ///   mkdir -p ~/.config/rdicom/
  ///   serve --print-config > ~/.config/rdicom/config.yaml
  #[arg(long, verbatim_doc_comment)]
  print_config: bool,
  // #[arg(short="V", long)]
  // version: bool,
  /// DICOM files root path (root path provided to the scan tool)
  #[arg(short = 'd', long)]
  dcmpath: String,
  /// Add some logs on the console
  #[arg(short, long)]
  verbose: bool,
  /// Log file path. No logs if not specified
  #[arg(short, long)]
  logfile: Option<String>,
  /// Insert a prefix between the base of the url and the path
  #[arg(short = 'x', long)]
  prefix: Option<String>,
}

#[derive(Debug)]
struct ApplicationError {
  message: String,
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
  let result = connection
    .prepare("PRAGMA table_info(dicom_index);")?
    .into_iter()
    .map(|row| row.map(|r| r.read::<&str, _>(1).to_string()))
    .collect::<Result<Vec<String>, _>>()?;
  Ok(result)
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
          Ok(tag) => {
            match tag.vr {
              "OB" | "OD" | "OF" | "OL" | "OV" | "OW" => {
                format!(
                  // Create a BulkdataURI
                  // "00080030": "/bulkdata/{StudyInstanceUID}/{SeriesInstanceUID}/{SOPInstanceUID}/{tag}",
                  "\"{:04X}{:04X}\": \"/bulkdata/{}\"",
                  tag.group, tag.element, value,
                )
              }
              _ => {
                format!(
                  // We have a Dicom that we will format according to the DicomWeb standard
                  // "00080030": {
                  //   "vr": "TM",
                  //   "Value": ["131600.0000"]
                  // },
                  "\"{:04X}{:04X}\": {{ \"vr\": \"{}\", \"Value\": [ \"{}\" ] }}",
                  // TODO: The replace here is an ugly workaround which is probably going to cause more
                  // problem than it will solve.
                  tag.group,
                  tag.element,
                  tag.vr,
                  value.replace("\\", ","),
                )
              }
            }
          }
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
trait ReadSeek: Read + Seek {}

trait InstanceFactory {
  fn get_reader(&self, path: &str) -> Result<Box<dyn ReadSeek>, DicomError>;
  fn get_writer(&self, path: &str, overwrite: bool) -> Result<Box<dyn Write>, DicomError>;
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

impl ReadSeek for BufReader<File> {}

impl InstanceFactory for FSInstanceFactory {
  fn get_reader(&self, path: &str) -> Result<Box<dyn ReadSeek>, DicomError> {
    let tmp = std::path::Path::new(&self.dcmpath).join(std::path::Path::new(path));
    let f = File::open(&*tmp.to_string_lossy())?;
    Ok(Box::new(BufReader::new(f)))
  }

  fn get_writer(&self, path: &str, overwrite: bool) -> Result<Box<dyn Write>, DicomError> {
    let path = std::path::Path::new(&self.dcmpath).join(std::path::Path::new(path));
    if !path.exists() || overwrite {
      let f = File::create(&*path.to_string_lossy())?;
      Ok(Box::new(BufWriter::new(f)))
    } else {
      error!("{:?} file already exists, cannot overwrite file", path);
      Err(DicomError::new("File already exists"))
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

impl ReadSeek for Cursor<Vec<u8>> {}

impl InstanceFactory for MemoryInstanceFactory {
  fn get_reader(&self, path: &str) -> Result<Box<dyn ReadSeek>, DicomError> {
    println!("<- {}", path);
    Ok(Box::new(Cursor::new(Vec::<u8>::new())))
  }
  fn get_writer(&self, path: &str, _overwrite: bool) -> Result<Box<dyn Write>, DicomError> {
    println!("-> {}", path);
    Ok(Box::new(Cursor::new(Vec::<u8>::new())))
  }
}

/**
 * Retrieve the fields from the index according to the search terms and enrich
 * the data from the index with the data from the DICOM files if necessary.
 */
fn get_entries(
  connection: &Connection,
  instance_factory: &Box<dyn InstanceFactory + Send + Sync>,
  params: &QidoQueryParameters,
  search_terms: &HashMap<Tag, String>,
  entry_type: &str,
) -> Result<Vec<HashMap<String, String>>, Box<dyn Error>> {
  let indexed_fields = get_indexed_fields(connection)?;
  // First retrieve the indexed fields present in the DB
  let query = &format!(
    "SELECT * FROM dicom_index {} GROUP BY {} {};",
    // Will restrict the data to what is being searched
    create_where_clause(params, search_terms, &indexed_fields),
    entry_type,
    create_limit_clause(params),
  );
  tracing::debug!("query: {}", query);
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

#[derive(Deserialize)]
struct SearchTerms {
  study_uid: Option<String>,
  series_uid: Option<String>,
  instance_uid: Option<String>,
}

#[axum_macros::debug_handler]
async fn get_studies(
  axum::extract::State(state): axum::extract::State<Arc<AppState>>,
  params: axum::extract::Query<QidoQueryParameters>,
  Path(SearchTerms {
    instance_uid,
    study_uid,
    series_uid,
  }): Path<SearchTerms>,
  headers: HeaderMap,
) -> impl IntoResponse {
  let mut search_terms = HashMap::<Tag, String>::new();
  if let Some(instance_uid) = instance_uid {
    search_terms.insert(dicom_tags::SOPInstanceUID, instance_uid);
  }
  if let Some(series_uid) = series_uid {
    search_terms.insert(dicom_tags::SeriesInstanceUID, series_uid);
  }
  if let Some(study_uid) = study_uid {
    search_terms.insert(dicom_tags::StudyInstanceUID, study_uid);
  }

  let mut response_headers = HeaderMap::new();
  match get_entries(
    &state.connection.lock().unwrap(),
    &state.instance_factory,
    &params,
    &search_terms,
    "StudyInstanceUID",
  ) {
    Ok(result) if result.len() > 0 => {
      let accept_formats = get_accept_formats(headers);
      if accept_formats
        .iter()
        .any(|e| e == "application/json" || e == "application/json+dicom")
      {
        response_headers.insert(
          "content-type",
          "application/dicom+json; charset=utf-8".parse().unwrap(),
        );
        // 🤮 TODO: need to replace generate_json_response
        (
          response_headers,
          generate_json_response(&result).into_response(),
        )
      } else {
        (
          response_headers,
          StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response(),
        )
      }
    }
    Ok(_) => (response_headers, StatusCode::NOT_FOUND.into_response()),
    Err(_) => (
      response_headers,
      StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    ),
  }
}

#[axum_macros::debug_handler]
async fn get_series(
  axum::extract::State(state): axum::extract::State<Arc<AppState>>,
  params: axum::extract::Query<QidoQueryParameters>,
  Path(SearchTerms {
    instance_uid,
    study_uid,
    series_uid,
  }): Path<SearchTerms>,
  headers: HeaderMap,
) -> impl IntoResponse {
  let mut search_terms = HashMap::<Tag, String>::new();
  if let Some(instance_uid) = instance_uid {
    search_terms.insert(dicom_tags::SOPInstanceUID, instance_uid);
  }
  if let Some(series_uid) = series_uid {
    search_terms.insert(dicom_tags::SeriesInstanceUID, series_uid);
  }
  if let Some(study_uid) = study_uid {
    search_terms.insert(dicom_tags::StudyInstanceUID, study_uid);
  }

  let mut response_headers = HeaderMap::new();
  match get_entries(
    &state.connection.lock().unwrap(),
    &state.instance_factory,
    &params,
    &search_terms,
    "SeriesInstanceUID",
  ) {
    Ok(result) if result.len() > 0 => {
      let accept_formats = get_accept_formats(headers);
      if accept_formats
        .iter()
        .any(|e| e == "application/json" || e == "application/json+dicom")
      {
        response_headers.insert(
          "content-type",
          "application/dicom+json; charset=utf-8".parse().unwrap(),
        );
        (
          response_headers,
          generate_json_response(&result).into_response(),
        )
      } else {
        (
          response_headers,
          StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response(),
        )
      }
    }
    Ok(_) => (response_headers, StatusCode::NOT_FOUND.into_response()),
    Err(_) => (
      response_headers,
      StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    ),
  }
}

#[axum_macros::debug_handler]
async fn get_instances(
  axum::extract::State(state): axum::extract::State<Arc<AppState>>,
  params: axum::extract::Query<QidoQueryParameters>,
  Path(SearchTerms {
    instance_uid,
    study_uid,
    series_uid,
  }): Path<SearchTerms>,
  headers: HeaderMap,
) -> impl IntoResponse {
  let mut search_terms = HashMap::<Tag, String>::new();
  if let Some(instance_uid) = instance_uid {
    search_terms.insert(dicom_tags::SOPInstanceUID, instance_uid);
  }
  if let Some(series_uid) = series_uid {
    search_terms.insert(dicom_tags::SeriesInstanceUID, series_uid);
  }
  if let Some(study_uid) = study_uid {
    search_terms.insert(dicom_tags::StudyInstanceUID, study_uid);
  }

  let mut response_headers = HeaderMap::new();
  match get_entries(
    &state.connection.lock().unwrap(),
    &state.instance_factory,
    &params,
    &search_terms,
    "filepath",
  ) {
    Ok(result) if result.len() > 0 => {
      let accept_formats = get_accept_formats(headers);
      if accept_formats
        .iter()
        .any(|e| e == "application/json" || e == "application/json+dicom")
      {
        response_headers.insert(
          "content-type",
          "application/dicom+json; charset=utf-8".parse().unwrap(),
        );
        (
          response_headers,
          generate_json_response(&result).into_response(),
        )
      } else {
        (
          response_headers,
          StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response(),
        )
      }
    }
    Ok(_) => (response_headers, StatusCode::NOT_FOUND.into_response()),
    Err(_) => (
      response_headers,
      StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    ),
  }
}

#[axum_macros::debug_handler]
async fn not_implemented(
  axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> impl IntoResponse {
  StatusCode::NOT_IMPLEMENTED.into_response()
}

#[axum_macros::debug_handler]
async fn not_found(
  axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> impl IntoResponse {
  StatusCode::NOT_FOUND.into_response()
}

fn get_study(ui: &str) -> HashMap<String, String> {
  HashMap::from([(String::from("link"), ui.to_string())])
}

fn get_serie(ui: &str) -> HashMap<String, String> {
  HashMap::from([(String::from("link"), ui.to_string())])
}

fn get_filepath(
  connection: &Connection,
  study_instance_uid: &str,
  series_instance_uid: &str,
  sop_instance_uid: &str,
) -> Result<String, Box<dyn Error>> {
  let query = &format!(
    "SELECT filepath FROM dicom_index WHERE StudyInstanceUID='{}' AND SeriesInstanceUID='{}' AND SOPInstanceUID='{}';",
    // Will restrict the data to what is being searched
    study_instance_uid,
    series_instance_uid,
    sop_instance_uid,
  );
  // println!("query: {}", query);
  return Ok(
    db::query(connection, query)?[0]
      .get("filepath")
      .ok_or("Entry not found")?
      .to_string(),
  );
}

// fn get_instance<T: InstanceFactory>(
//   connection: &Connection,
//   instance_factory: &T,
//   search_terms: &HashMap<Tag, String>,
// ) -> Result<Vec<HashMap<String, String>>, Box<dyn Error>> {
//   // Retrieve the filename of the instances matching the search parameters
//   let study_instance_uid = search_terms
//     .get(&dicom_tags::StudyInstanceUID)
//     .ok_or("Missing StudyInstanceUID in search terms")?;
//   let series_instance_uid = search_terms
//     .get(&dicom_tags::SeriesInstanceUID)
//     .ok_or("Missing SeriesInstanceUID in search terms")?;
//   let sop_instance_uid = search_terms
//     .get(&dicom_tags::SOPInstanceUID)
//     .ok_or("Missing SOPInstanceUID in search terms")?;
//   let filepath = get_filepath(
//     connection,
//     study_instance_uid,
//     series_instance_uid,
//     sop_instance_uid,
//   )?;
//   // Load the file
//   let mut result = HashMap::<String, String>::new();
//   result.insert("filepath".to_string(), filepath.clone());
//   let reader = instance_factory.get_reader(&filepath)?;
//   let instance = Instance::from_reader(reader)?;
//   // Go through all the fields of the instance
//   for attribute in instance.iter() {
//     if let Ok(attribute) = attribute {
//       if attribute.tag.vr != "UN" {
//         // TODO: Better management of unknown tags
//         if let Ok(value) = DicomValue::from_dicom_attribute(&attribute, &instance) {
//           match value {
//             // TODO: Manage nested fields
//             DicomValue::SQ(_)
//             // TODO: Manage BulkdataURI
//             | DicomValue::OB(_)
//             | DicomValue::OD(_)
//             | DicomValue::OF(_)
//             | DicomValue::OL(_)
//             | DicomValue::OV(_)
//             | DicomValue::OW(_) => {
//               result.insert(
//                 attribute.tag.to_string(),
//                 format!("{}/{}/{}/{:04x}{:04x}", study_instance_uid, series_instance_uid,
//                   sop_instance_uid, attribute.tag.group, attribute.tag.element),
//               );
//             },
//             // TODO: Better management of unknown tags
//             DicomValue::UN(_) => (),
//             _ => {
//               result.insert(attribute.tag.to_string(), value.to_string());
//             },
//           }
//         }
//       }
//     }
//   }
//   Ok(vec![result])
// }

fn get_bulk_tag<T: InstanceFactory>(
  connection: &Connection,
  instance_factory: &T,
  search_terms: &HashMap<Tag, String>,
  tag: Tag,
) -> Result<Vec<u8>, Box<dyn Error>> {
  let study_instance_uid = search_terms
    .get(&Tag::try_from("StudyInstanceUID")?)
    .ok_or("Missing StudyInstanceUID in search terms")?;
  let series_instance_uid = search_terms
    .get(&Tag::try_from("SeriesInstanceUID")?)
    .ok_or("Missing SeriesInstanceUID in search terms")?;
  let sop_instance_uid = search_terms
    .get(&Tag::try_from("SOPInstanceUID")?)
    .ok_or("Missing SOPInstanceUID in search terms")?;
  let filepath = get_filepath(
    connection,
    study_instance_uid,
    series_instance_uid,
    sop_instance_uid,
  )?;
  let instance = Instance::from_reader(instance_factory.get_reader(&filepath)?)?;
  match instance.get_value(&tag) {
    Ok(Some(dicom_value)) => match dicom_value {
      DicomValue::OB(value) => Ok(value.to_owned()),
      DicomValue::OD(_) => Ok(vec![]),
      DicomValue::OF(_) => Ok(vec![]),
      DicomValue::OL(_) => Ok(vec![]),
      DicomValue::OV(_) => Ok(vec![]),
      DicomValue::OW(value) => Ok(vec![]),
      _ => Err(format!("Unsupported bulkdata tag {:?}", tag).into()),
    },
    Ok(None) => Err(format!("No such tag {:?}", tag).into()),
    Err(e) => Err(Box::new(e)),
  }
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

fn dicom_attribute_json_to_string(attribute: &DicomAttributeJson) -> String {
  String::try_from(attribute.payload.clone().unwrap()).unwrap_or("undefined".to_string())
}

fn delete_all_studies(connection: &Connection) -> Result<(), Box<dyn Error>> {
  db::query(connection, "DELETE FROM dicom_index;")?;
  Ok(())
}

// Convert a IntoResponse to ApiError
// Used with `WithRejection`
// from: https://github.com/tokio-rs/axum/blob/main/examples/customize-extractor-error/src/with_rejection.rs
mod custom_error {
  use crate::IntoResponse;
  use crate::JsonRejection;
  use crate::StatusCode;
  use axum::Json;
  use serde_json::json;

  pub struct ApiError {
    status: StatusCode,
    error: String,
  }

  impl From<JsonRejection> for ApiError {
    fn from(rejection: JsonRejection) -> ApiError {
      return ApiError {
        status: rejection.status(),
        error: rejection.body_text(),
      };
    }
  }

  // We implement `IntoResponse` so ApiError can be used as a response
  impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
      let payload = json!({
        "error": self.error,
      });

      (self.status, Json(payload)).into_response()
    }
  }
}

#[axum_macros::debug_handler]
async fn post_studies(
  axum::extract::State(state): axum::extract::State<Arc<AppState>>,
  Path(SearchTerms {
    study_uid,
    series_uid,
    instance_uid,
  }): Path<SearchTerms>,
  // TODO: Find a way to handle different Content-Type. Now we assume that Content-Type is json
  WithRejection(Json(dataset), _): WithRejection<
    Json<BTreeMap<String, DicomAttributeJson>>,
    custom_error::ApiError,
  >,
) -> axum::response::Result<()> {
  let sop_instance_uid = String::try_from(
    dataset
      .get(&dicom_tags::SOPInstanceUID.to_string())
      .unwrap()
      .payload
      .clone()
      .unwrap(),
  )
  .map_err(|e| {
    tracing::error!(e.details);
    (
      StatusCode::BAD_REQUEST,
      Json("Could not perform STORE").into_response(),
    )
  })?;
  let filename = &format!("{}.dcm", sop_instance_uid);
  let overwrite = state.config.store_overwrite.unwrap_or(false);
  let mut writer = state
    .instance_factory
    .get_writer(filename, overwrite)
    .map_err(|e| {
      tracing::warn!("Could not get a writer for {}: {}", filename, e);
      (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json("Could not perform STORE").into_response(),
      )
    })?;
  // Write the file
  tracing::debug!("writing {}", filename);
  json2dcm::json2dcm(&mut writer, &dataset).map_err(|e| {
    tracing::warn!(
      "Error while streaming the dicom file to {}: {}",
      filename,
      e
    );
    (
      StatusCode::INTERNAL_SERVER_ERROR,
      Json("Could not perform STORE").into_response(),
    )
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
  state
    .index_store
    .lock()
    .unwrap()
    .write(&data)
    .map_err(|e| {
      tracing::warn!(
        "Error while updating the index file to the DB for {}: {}",
        filename,
        e
      );
      (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json("Could not perform STORE").into_response(),
      )
    })?;
  Ok(())
}

#[axum_macros::debug_handler]
async fn delete_studies(
  axum::extract::State(state): axum::extract::State<Arc<AppState>>,
  Path(SearchTerms {
    study_uid,
    series_uid,
    instance_uid,
  }): Path<SearchTerms>,
) -> impl IntoResponse {
  let query = if let Some(study_uid) = study_uid {
    &format!(
      "DELETE FROM dicom_index WHERE StudyInstanceUID == {};",
      study_uid
    )
  } else {
    "DELETE FROM dicom_index;"
  };
  match db::query(&state.connection.lock().unwrap(), query) {
    Ok(_) => StatusCode::OK.into_response(),
    Err(e) => {
      tracing::error!(e);
      Json("Could not perform delete").into_response()
    }
  }
}

// If configuration was provided, we check the database respects the config. If
// the database does not exists we create it with respect to the provided
// config. If no configuration was provided, we check the database exists with a
// 'dicom_index' table.
fn check_db(
  opt: &Opt,
  config: &config::Config,
) -> Result<(Vec<String>, ConnectionThreadSafe), Box<dyn Error>> {
  let connection = Connection::open_thread_safe(&opt.sqlfile)?;

  let mut indexable_fields = config.get_indexable_fields();
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
  Ok((indexable_fields, connection))
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

fn get_accept_formats(headers: HeaderMap) -> Vec<String> {
  // The following 3 lines could be within a function
  let accept_types = headers
    .get(ACCEPT)
    .and_then(|ct| ct.to_str().ok().map(String::from))
    .unwrap_or_else(|| "*/*".to_string())
    .split(",")
    .map(|s| s.trim().to_string())
    .collect::<Vec<String>>();

  return accept_types;
}

struct AppState {
  // TODO: Rework index_store so that we do not need an Arc Mutex here
  connection: Arc<Mutex<ConnectionThreadSafe>>,
  index_store: Arc<Mutex<SqlIndexStoreWithMutex>>,
  instance_factory: Box<dyn InstanceFactory + Sync + Send>,
  config: config::Config,
}

async fn print_request_response(
  req: Request,
  next: Next,
) -> Result<impl IntoResponse, (StatusCode, String)> {
  let (parts, body) = req.into_parts();
  let bytes = buffer_and_print("request", body).await?;
  let req = Request::from_parts(parts, Body::from(bytes));

  let res = next.run(req).await;

  let (parts, body) = res.into_parts();
  let bytes = buffer_and_print("response", body).await?;
  let res = Response::from_parts(parts, Body::from(bytes));

  Ok(res)
}

async fn buffer_and_print<B>(direction: &str, body: B) -> Result<Bytes, (StatusCode, String)>
where
  B: axum::body::HttpBody<Data = Bytes>,
  B::Error: std::fmt::Display,
{
  let bytes = match body.collect().await {
    Ok(collected) => collected.to_bytes(),
    Err(err) => {
      return Err((
        StatusCode::BAD_REQUEST,
        format!("failed to read {direction} body: {err}"),
      ));
    }
  };

  if let Ok(body) = std::str::from_utf8(&bytes) {
    tracing::debug!("{direction} body = {body:?}");
  }

  Ok(bytes)
}

fn get_first_accept_formats(
  accept_formats: &Vec<String>,
  proposed_formats: &[&str],
) -> Option<String> {
  if accept_formats.len() == 0 {
    return Some(proposed_formats[0].to_string());
  }
  for accept_format in accept_formats.iter() {
    if proposed_formats.iter().any(|e| e == accept_format) {
      return Some(accept_format.clone());
    }
  }
  if accept_formats.iter().any(|e| e == "*/*") {
    return Some(proposed_formats[0].to_string());
  }
  None
}

#[axum_macros::debug_handler]
async fn get_capabilities(
  axum::extract::State(state): axum::extract::State<Arc<AppState>>,
  headers: HeaderMap,
) -> impl IntoResponse {
  let mut response_headers = HeaderMap::new();
  let accept_formats = get_accept_formats(headers);
  match get_first_accept_formats(
    &accept_formats,
    &[
      "application/vnd.sun.wadl+xml",
      "application/dicom+xml",
      "application/xml",
      "application/json",
      "application/dicom+json",
    ],
  ) {
    Some(format)
      if format == "application/vnd.sun.wadl+xml"
        || format == "application/dicom+xml"
        || format == "application/xml" =>
    {
      response_headers.insert("content-type", "application/dicom+xml".parse().unwrap());
      (
        response_headers,
        capabilities::CAPABILITIES_STR.into_response(),
      )
    }
    Some(format) if format == "application/json" || format == "application/dicom+json" => {
      response_headers.insert("content-type", "application/dicom+json".parse().unwrap());
      let application =
        quick_xml::de::from_str::<capabilities::Application>(capabilities::CAPABILITIES_STR)
          .unwrap();
      // Because of https://github.com/tafia/quick-xml/issues/582, json output
      // is polluted with field names starting with "@". We replace them here.
      // TODO: Write a intermediary serializer to handle these.
      (
        response_headers,
        serde_json::to_string(&application)
          .unwrap()
          .replace('@', "")
          .into_response(),
      )
    }
    Some(_) | None => (
      response_headers,
      StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response(),
    ),
  }
}

fn get_config(opt: &Opt) -> Result<config::Config, Box<dyn Error>> {
  let config_access = config_file::get_config(&opt.config, DEFAULT_CONFIG)?;

  if opt.print_config {
    match config_access.provenance {
      ConfigProvenance::Default => eprintln!("# Default internal config\n"),
      ConfigProvenance::XdgPath(path) => eprintln!("# Default config file {}\n", path),
      ConfigProvenance::CustomPath(path) => {
        eprintln!("# Config file provided by command line {}\n", path)
      }
    }
    // Print the content of the configuration file and exit
    println!("{}", config_access.content);
    std::process::exit(0);
  }

  let config: config::Config = serde_yaml::from_str(&config_access.content)?;

  Ok(config)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  // Retrieve options
  let opt = Opt::parse();

  // Configure a custom event formatter
  let format = tracing_subscriber::fmt::format()
    .with_level(true)
    .with_target(false)
    .compact();
  let subscriber = tracing_subscriber::fmt()
    .event_format(format)
    .with_max_level(if opt.verbose {
      Level::TRACE
    } else {
      Level::INFO
    })
    .finish();
  tracing::subscriber::set_global_default(subscriber).unwrap();

  // Load a config file if any
  let config: config::Config = get_config(&opt)?;

  // Check the the status of the database and the option are coherent.
  let (indexable_fields, connection) = check_db(&opt, &config)?;
  let connection = Arc::new(Mutex::new(connection));
  let index_store =
    SqlIndexStoreWithMutex::new(connection.clone(), &config.table_name, indexable_fields)?;

  let instance_factory: Box<dyn InstanceFactory + Sync + Send> = if opt.dcmpath == ":memory:" {
    Box::new(MemoryInstanceFactory::new())
  } else {
    Box::new(FSInstanceFactory::new(&opt.dcmpath))
  };

  // TODO: Add this header to all response
  // let server_header: &'static str = concat!("rdicomweb/", env!("CARGO_PKG_VERSION"));
  // TODO: ??
  static APPLICATION: Lazy<capabilities::Application> =
    Lazy::new(|| quick_xml::de::from_str(capabilities::CAPABILITIES_STR).unwrap());
  // TODO: ??
  let prefix = opt.prefix.unwrap_or("".to_string());

  let app_state = AppState {
    connection: connection,
    index_store: Arc::new(Mutex::new(index_store)),
    instance_factory: instance_factory,
    config: config,
  };

  // Build our application with a route
  let mut app = Router::new()
    .route("/", get(get_capabilities))
    .route("/", options(get_capabilities))
    .route("/about", get(|| async { SERVER_HEADER }))
    // GET
    .route("/instances", get(get_instances))
    .route("/instances/{instance_uid}", get(get_instances))
    .route(
      "/instances/{instance_uid}/frames/{frame_uid}",
      get(not_implemented),
    )
    .route("/instances/{instance_uid}/rendered", get(not_implemented))
    .route("/instances/{instance_uid}/thumbnail", get(not_implemented))
    .route("/instances/{instance_uid}/{tag_id}", get(not_found))
    .route("/series", get(get_series))
    .route("/series/{series_uid}", get(get_series))
    .route("/series/{series_uid}/instances", get(get_instances))
    .route(
      "/series/{series_uid}/instances/{instance_uid}",
      get(get_instances),
    )
    .route(
      "/series/{series_uid}/instances/{instance_uid}/frames/{frame_uid}",
      get(not_implemented),
    )
    .route(
      "/series/{series_uid}/instances/{instance_uid}/rendered",
      get(not_implemented),
    )
    .route(
      "/series/{series_uid}/instances/{instance_uid}/thumbnail",
      get(not_implemented),
    )
    .route(
      "/series/{series_uid}/instances/{instance_uid}/{tag_id}",
      get(not_implemented),
    )
    .route("/studies", get(get_studies))
    .route("/studies/{study_uid}", get(get_studies))
    .route("/studies/{study_uid}/series", get(get_series))
    .route("/studies/{study_uid}/series/{series_uid}", get(get_series))
    .route(
      "/studies/{study_uid}/series/{series_uid}/instances",
      get(get_instances),
    )
    .route(
      "/studies/{study_uid}/series/{series_uid}/instances/{instances_uid}",
      get(get_instances),
    )
    .route(
      "/studies/{study_uid}/series/{series_uid}/instances/{instances_uid}/frames/{frame_uid}",
      get(not_implemented),
    )
    .route(
      "/studies/{study_uid}/series/{series_uid}/instances/{instances_uid}/rendered",
      get(not_implemented),
    )
    .route(
      "/studies/{study_uid}/series/{series_uid}/instances/{instances_uid}/thumbnail",
      get(not_implemented),
    )
    .route(
      "/studies/{study_uid}/series/{series_uid}/instances/{instances_uid}/{tag_id}",
      get(not_implemented),
    )
    // POST
    .route("/studies", post(post_studies))
    .route("/studies/{study_uid}", post(not_implemented))
    // DELETE (not part of DICOMWeb)
    .route("/studies", delete(delete_studies))
    .route("/studies/{study_uid}", delete(delete_studies))
    .layer(middleware::from_fn(print_request_response))
    .with_state(Arc::new(app_state));

  let host = opt.host;
  println!(
    "Serving HTTP on {} port {} (dicom: http://{}:{}/{}) with database {} ...",
    host, opt.port, host, opt.port, &prefix, opt.sqlfile
  );
  info!(
    "Serving HTTP on {} port {} (dicom: http://{}:{}/{}) with database {} ...",
    host, opt.port, host, opt.port, &prefix, opt.sqlfile
  );

  // Add some logging on each request/response
  app = app.layer(
    TraceLayer::new_for_http()
      .make_span_with(trace::DefaultMakeSpan::new().level(Level::INFO))
      .on_response(
        trace::DefaultOnResponse::new()
          .level(Level::INFO)
          .include_headers(true),
      ),
  );

  // run our app with hyper
  let listener = tokio::net::TcpListener::bind(format!("{}:{}", host, opt.port))
    .await
    .unwrap();
  tracing::debug!("listening on {}", listener.local_addr().unwrap());

  axum::serve(listener, app).await.unwrap();
  Ok(())
}

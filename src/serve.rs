#![allow(unused_variables)]
#![allow(dead_code)]

use std::convert::TryInto;
use rdicom::error::DicomError;
use std::convert::Infallible;
use std::collections::HashMap;
use std::error::Error;
use structopt::StructOpt;
use std::path::PathBuf;
use warp::{Filter, reject, Rejection};
use std::net::IpAddr;
use std::str::FromStr;
use serde::{de, Deserialize, Deserializer, Serialize};
use serde_json;
use std::fmt;
use rusqlite::{Connection, params};
use warp::http::header::{CONTENT_TYPE, HeaderMap};

use rdicom::tags::Tag;

mod config;

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

#[derive(Debug, StructOpt)]
struct Opt {
    #[structopt(short, long)]
    port: Option<u16>,

    #[structopt(short, long)]
    host: Option<String>,

    #[structopt(short, long, parse(try_from_str = file_exists))]
    sqlfile: PathBuf,
}

#[derive(Debug)]
struct NotAUniqueIdentifier;
impl reject::Reject for NotAUniqueIdentifier {}

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

fn search_terms() -> impl Filter<Extract = (HashMap<Tag, String>,), Error = Rejection> + Copy {
  warp::query::<HashMap<String, String>>().and_then(|q: HashMap<String, String>| async move {
    if true {
      Ok(q.into_iter()
        .filter_map(|(k, v)| if let Some(tag) = TryInto::<Tag>::try_into(&k).ok() {
          Some((tag, v))
        } else {
          None
        })
        // .map(|(k, v)| ((&k).try_into().unwrap(), v))
        .collect::<HashMap<Tag, String>>())
    } else { // TODO: Without the else clause, rust complains. Need to figure out why.
      Err(reject::custom(NotAUniqueIdentifier))
    }
  })
}


// For some reason, serde can't deserialize an array of String, so we provide a
// custom function that do so.
fn deserialize_array<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
  D: Deserializer<'de> {

  struct VectorStringVisitor;

  impl<'de> de::Visitor<'de> for VectorStringVisitor {
    type Value = Option<Vec<String>>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
      formatter.write_str("a vector of string")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
      E: de::Error {
      Ok(Some(v.split(',').map(|s| String::from(s)).collect::<Vec<String>>()))
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

// Retrieves the column present in the index
fn get_indexed_fields(connection: &Connection) -> Vec<String> {
  connection.prepare("PRAGMA table_info(dicom_index);").and_then(|mut stmt| {
    Ok(stmt.query_map(params![], |row| {
      Ok(row.get(1))
    })?.map(|x| x.unwrap().unwrap()).collect::<Vec<String>>())
  }).unwrap()
}

// Performs an arbitrary query on the connection
fn query(connection: &Connection, query: &str) -> Vec<HashMap<String, String>> {
  connection.prepare(query).and_then(|mut stmt| {
    Ok(stmt.query_map(params![], |row| {
      let mut entries = HashMap::new();
      for (index, column_name) in get_indexed_fields(connection).iter().enumerate() {
        let value: String = row.get(index).unwrap();
        if value != "undefined" {
          entries.insert(column_name.to_owned(), value);
        }
      }
      Ok(entries)
    })?.map(|x| x.unwrap()).collect::<_>())
  }).unwrap()
}

fn map_to_entry(tag_map: &HashMap<String, String>) -> String {
  format!("{{ {} }}", tag_map.iter()
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
          tag.group, tag.element, tag.vr, value,
        ),
        // Otherwise, just dump the key in the object
        _ => format!("\"{key}\": \"{value}\""),
      }
    })
    .collect::<Vec<String>>()
    .join(",")
  )
}

fn create_where_clause(params: &QidoQueryParameters, search_terms: &HashMap<Tag, String>,
  indexed_fields: &Vec<String>) -> String {
  // limit
  // offset
  // fuzzymatching
  // includefield
  let limit = params.limit.unwrap_or(u32::MAX as usize);
  let offset = params.offset.unwrap_or(0);
  let fuzzymatching = params.fuzzymatching.unwrap_or(false);

  let where_clause = search_terms.iter()
    .filter(|(field, _)| indexed_fields.contains(&field.name.to_owned()))
    .fold(String::new(), |mut acc, (field, value)| {
      if acc.len() == 0 {
        acc += "WHERE ";
      }
      acc + &format!("{}{}{}{}", field.name,
        if fuzzymatching { " LIKE '%" } else { "='" },
        value,
        if fuzzymatching { "%'" } else { "'" },)
    });
  format!("{where_clause} LIMIT {limit} OFFSET {offset}")
}

fn get_studies(connection: &Connection, params: &QidoQueryParameters,
  search_terms: &HashMap<Tag, String>) -> Vec<HashMap<String, String>> {
  let indexed_fields = get_indexed_fields(connection);
  // First retrieve the indexed fields
  let indexed = query(&connection,
    &format!("SELECT DISTINCT StudyInstanceUID, * FROM dicom_index {};",
      create_where_clause(params, search_terms, &indexed_fields)));
  // Get the includefields not present in the index
  if let Some(includefield) = params.includefield {
    
  }
  // Then enrich them with the fields from the DICOM fields
  // TODO
  return indexed;
}

fn get_series(params: &QidoQueryParameters) -> Vec<String> {
  vec![]
}

fn get_instances(params: &QidoQueryParameters) -> Vec<String> {
  vec![]
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

/**
 * https://www.dicomstandard.org/using/dicomweb/query-qido-rs/
 */
fn get_query_api(sqlfile: String)
  -> impl Filter<Extract = impl warp::Reply, Error = Rejection> + Clone {
  // No literal constructor for HeaderMap, so have to allocate them here...
  let mut json_headers = HeaderMap::new();
  json_headers.insert(CONTENT_TYPE, "application/dicom+json; charset=utf-8".parse().unwrap());

  // GET {s}/studies?... Query for studies
  let studies = warp::path("studies")
    .and(warp::path::end())
    .and(warp::query::<QidoQueryParameters>())
    .and(search_terms())
    .and(with_db(sqlfile))
    .map(|qido_params: QidoQueryParameters, search_terms: HashMap<Tag, String>, connection: Connection| {
      // serde_json::to_string(&get_studies(&connection, &qido_params)).unwrap()
      format!("[{}]", get_studies(&connection, &qido_params, &search_terms).iter()
        .map(|study| map_to_entry(study))
        .collect::<Vec<String>>()
        .join(",")
      )
    })
    .with(warp::reply::with::headers(json_headers));

  // GET {s}/studies/{study}/series?...  Query for series in a study
  let series = warp::path("series")
    .and(warp::path::end())
    .and(warp::query::<QidoQueryParameters>())
    .map(|params: QidoQueryParameters| {
      serde_json::to_string(&get_series(&params)).unwrap()
    });

  // GET {s}/studies/{study}/series/{series}/instances?... Query for instances in a series
  let instances = warp::path("instances")
    .and(warp::path::end())
    .and(warp::query::<QidoQueryParameters>())
    .map(|params: QidoQueryParameters| {
      serde_json::to_string(&get_instances(&params)).unwrap()
    });

  let studies_series = warp::path("studies")
    .and(unique_identifier())
    .and(warp::path("series"))
    .and(warp::query::<QidoQueryParameters>())
    .map(|study_uid: String, params: QidoQueryParameters| {
      serde_json::to_string(&get_instance(&study_uid)).unwrap()
    });

  let studies_series_instances = warp::path("studies")
    .and(unique_identifier())
    .and(warp::path("series"))
    .and(unique_identifier())
    .and(warp::path("instances"))
    .and(warp::query::<QidoQueryParameters>())
    .map(|study_uid: String, series_uid: String, params: QidoQueryParameters| {
      serde_json::to_string(&get_instances(&params)).unwrap()
    });

  warp::get()
    .and(studies)
    .or(series)
    .or(instances)
    .or(studies_series)
    .or(studies_series_instances)
}

fn with_db<'a>(sqlfile: String) -> impl Filter<Extract = (Connection,), Error = Infallible> + Clone + 'a {
    warp::any().map(move || Connection::open(&sqlfile).unwrap())
}

/**
 * https://www.dicomstandard.org/using/dicomweb/retrieve-wado-rs-and-wado-uri/
 */
fn get_retrieve_api(sqlfile: String)
  -> impl Filter<Extract = impl warp::Reply, Error = Rejection> + Clone {
  // GET {s}/studies/{study} Retrieve entire study
  let studies = warp::path("studies")
    .and(unique_identifier())
    .and(warp::path::end())
    .map(|study_uid: String| {
      serde_json::to_string(&get_study(&study_uid)).unwrap()
    });
  // GET {s}/studies/{study}/rendered  Retrieve rendered study
  let studies_rendered = warp::path("studies")
    .and(unique_identifier())
    .and(warp::path("rendered"))
    .and(warp::query::<WadoQueryParameters>())
    .and(warp::path::end())
    .map(|study_uid: String, params: WadoQueryParameters| {
      serde_json::to_string("").unwrap()
    });
  // GET {s}/studies/{study}/series/{series} Retrieve entire series
  let studies_series = warp::path("studies")
    .and(unique_identifier())
    .and(warp::path("series"))
    .and(unique_identifier())
    .and(warp::path::end())
    .map(|study_uid: String, series_uid: String| {
      serde_json::to_string("").unwrap()
    });
  // GET {s}/studies/{study}/series/{series}/rendered  Retrieve rendered series
  let studies_series_rendered = warp::path("studies")
    .and(unique_identifier())
    .and(warp::path("series"))
    .and(unique_identifier())
    .and(warp::path("rendered"))
    .and(warp::query::<WadoQueryParameters>())
    .and(warp::path::end())
    .map(|study_uid: String, series_uid: String, params: WadoQueryParameters| {
      serde_json::to_string("").unwrap()
    });
  // GET {s}/studies/{study}/series/{series}/metadata  Retrieve series metadata
  let studies_series_metadata = warp::path("studies")
    .and(unique_identifier())
    .and(warp::path("series"))
    .and(unique_identifier())
    .and(warp::path("metadata"))
    .and(warp::path::end())
    .map(|study_uid: String, series_uid: String| {
      serde_json::to_string("").unwrap()
    });

  let series = warp::path("series")
    .and(unique_identifier())
    .and(warp::path::end())
    .map(|study_uid: String| {
      serde_json::to_string(&get_study(&study_uid)).unwrap()
    });
  let series_rendered = warp::path("series")
    .and(unique_identifier())
    .and(warp::path("rendered"))
    .and(warp::query::<WadoQueryParameters>())
    .and(warp::path::end())
    .map(|study_uid: String, params: WadoQueryParameters| {
      serde_json::to_string("").unwrap()
    });

  // GET {s}/studies/{study}/series/{series}/instances/{instance}  Retrieve instance
  let studies_series_instances = warp::path("studies")
    .and(unique_identifier())
    .and(warp::path("series"))
    .and(unique_identifier())
    .and(warp::path("instances"))
    .and(unique_identifier())
    .and(warp::path::end())
    .map(|study_uid: String, series_uid: String, instance_uid: String| {
      serde_json::to_string("").unwrap()
    });
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
    .map(|study_uid: String, series_uid: String, instance_uid: String, params: WadoQueryParameters| {
      serde_json::to_string("").unwrap()
    });
  // GET {s}/studies/{study}/series/{series}/instances/{instance}/metadata Retrieve instance metadata
  let studies_series_instances_metadata = warp::path("studies")
    .and(unique_identifier())
    .and(warp::path("series"))
    .and(unique_identifier())
    .and(warp::path("instances"))
    .and(unique_identifier())
    .and(warp::path("metadata"))
    .and(warp::path::end())
    .map(|study_uid: String, series_uid: String, instance_uid: String| {
      serde_json::to_string("").unwrap()
    });

  let instance = warp::path("instances")
    .and(unique_identifier())
    .and(warp::path::end())
    .map(|instance_uid: String| {
      serde_json::to_string(&get_instance(&instance_uid)).unwrap()
    });

  let instance_rendered = warp::path("instances")
    .and(unique_identifier())
    .and(warp::path("rendered"))
    .and(warp::query::<WadoQueryParameters>())
    .and(warp::path::end())
    .map(|instance_uid: String, params: WadoQueryParameters| {
      serde_json::to_string(&get_instance(&instance_uid)).unwrap()
    });

  // GET {s}/studies/{study}/series/{series}/instances/{instance}/frames/{frames}  Retrieve frames in an instance
  let studies_series_instances = warp::path("studies")
    .and(unique_identifier())
    .and(warp::path("series"))
    .and(unique_identifier())
    .and(warp::path("instances"))
    .and(unique_identifier())
    .and(warp::path::end())
    .map(|study_uid: String, series_uid: String, instance_uid: String| {
      serde_json::to_string("").unwrap()
    });

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  // Retrieve options
  let opt = Opt::from_args();
  // Load the config
  // let config_file = std::fs::read_to_string(&opt.config)?;
  // let config: config::Config = serde_yaml::from_str(&config_file)?;

  let sqlfile = opt.sqlfile.to_string_lossy().to_string();

  // GET /
  let root = warp::get().and(warp::path::end()).map(|| "DICOM Web Server");
  let log = warp::log::custom(|info| {
    eprintln!("{} {} {}", info.method(), info.path(), info.status());
  });

  let routes = root
    .or(get_query_api(sqlfile.clone()))
    .or(get_retrieve_api(sqlfile.clone()))
    .with(warp::cors().allow_any_origin())
    .with(log)
  ;

  let host = opt.host.unwrap_or(String::from("127.0.0.1"));
  let port = opt.port.unwrap_or(8080);

  println!("Serving HTTP on {} port {} (http://{}:{}/) with database {:?} ...",
    host, port, host, port, &opt.sqlfile);
  warp::serve(routes).run((IpAddr::from_str(&host).unwrap(), port)).await;

  Ok(())
}

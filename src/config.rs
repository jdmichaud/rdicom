use serde::{Deserialize};

#[derive(Deserialize, Debug)]
pub struct Fields {
  pub studies: Vec<String>,
  pub series: Vec<String>,
  pub instances: Vec<String>,
}

#[derive(Deserialize, Debug)]
pub struct Indexing {
  pub fields: Fields,
}

#[derive(Deserialize, Debug)]
pub struct Config {
  pub indexing: Indexing,
  pub table_name: String,
}

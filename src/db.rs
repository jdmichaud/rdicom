use crate::HashMap;
use sqlite::{Connection, State};
use std::error::Error;

// Performs an arbitrary query on the connection
pub fn query(connection: &Connection, query: &str) -> Result<Vec<HashMap<String, String>>, Box<dyn Error>> {
  // println!("query {}", query);
  // TODO: Remove unwrap
  let mut statement = connection.prepare(query)?;
  let mut result: Vec<HashMap<String, String>> = Vec::new();
  while let Ok(State::Row) = statement.next() {
    let column_names = statement.column_names();
    let mut entries = HashMap::new();
    for column_name in column_names {
      entries.insert(column_name.to_owned(), statement.read::<String, _>(&**column_name)?);
    }
    result.push(entries);
  }

  return Ok(result);
}

use sqlite::{Connection, State};
use std::collections::HashMap;
use std::error::Error;

// Performs an arbitrary query on the connection
pub fn query(
  connection: &Connection,
  query: &str,
) -> Result<Vec<HashMap<String, String>>, Box<dyn Error>> {
  // println!("query {}", query);
  let mut statement = connection.prepare(query)?;
  let mut result: Vec<HashMap<String, String>> = Vec::new();
  // Propagate errors (e.g. a busy database) instead of returning partial results
  while let State::Row = statement.next()? {
    let column_names = statement.column_names();
    let mut entries = HashMap::with_capacity(column_names.len());
    for (index, column_name) in column_names.iter().enumerate() {
      entries.insert(column_name.to_owned(), statement.read::<String, _>(index)?);
    }
    result.push(entries);
  }

  Ok(result)
}

use crate::HashMap;
use sqlite::{Connection, State};

// Performs an arbitrary query on the connection
pub fn query(connection: &Connection, query: &str) -> Vec<HashMap<String, String>> {
  // println!("query {}", query);
  // TODO: Remove unwrap
  let mut statement = connection.prepare(query).unwrap();
  let mut result: Vec<HashMap<String, String>> = Vec::new();
  while let Ok(State::Row) = statement.next() {
    let column_names = statement.column_names();
    let mut entries = HashMap::new();
    for column_name in column_names {
      entries.insert(column_name.to_owned(), statement.read::<String, _>(&**column_name).unwrap());
    }
    result.push(entries);
  }

  return result;
}

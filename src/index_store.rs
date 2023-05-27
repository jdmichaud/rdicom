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

use sqlite::Connection;
use std::collections::HashMap;
use std::error::Error;
use std::io::Write;
use std::sync::Arc;
use std::sync::Mutex;

use crate::db;

pub trait IndexStore {
  fn begin_transaction(&self) -> Result<(), Box<dyn Error>>;
  fn end_transaction(&self) -> Result<(), Box<dyn Error>>;
  fn write(&mut self, data: &HashMap<String, String>) -> Result<(), Box<dyn Error>>;
}

#[derive(Debug)]
pub struct CsvIndexStore<W: Write> {
  writer: W,
  fields: Vec<String>,
}

impl<W: Write> CsvIndexStore<W> {
  pub fn new(mut writer: W, fields: Vec<String>) -> Self {
    let header = fields
      .iter()
      .map(|s| String::from("\"") + s + "\"")
      .collect::<Vec<String>>()
      .join(",");
    writeln!(writer, "").unwrap();
    CsvIndexStore { writer, fields }
  }
}

impl<W: Write> IndexStore for CsvIndexStore<W> {
  fn begin_transaction(&self) -> Result<(), Box<dyn Error>> {
    Ok(())
  }
  fn end_transaction(&self) -> Result<(), Box<dyn Error>> {
    Ok(())
  }

  fn write(self: &mut Self, data: &HashMap<String, String>) -> Result<(), Box<dyn Error>> {
    for field in &self.fields {
      match write!(
        self.writer,
        "\"{}\",",
        data.get(field).unwrap_or(&"undefined".to_string())
      ) {
        Ok(_) => (),
        Err(e) => return Err(Box::new(e)),
      }
    }
    writeln!(self.writer, "")?;
    Ok(())
  }
}

// Look for the entry in the DB, update it if present, create it otherwise. This makes
// scan reentrant when using an SQL store.
fn write_data(
  connection: &Connection,
  table_name: &String,
  fields: &Vec<String>,
  data: &HashMap<String, String>,
) -> Result<(), Box<dyn Error>> {
  // Check if the UIDs are not already present in the database
  let uid_fields = fields.iter().filter(|f| f.to_uppercase().ends_with("UID"));
  let constraints = uid_fields
    .map(|f| {
      format!(
        "{}=\"{}\"",
        f,
        data.get(f).unwrap_or(&"undefined".to_string())
      )
    })
    .collect::<Vec<String>>()
    .join(" AND ");
  let already_present = db::query(
    &connection,
    &format!("SELECT * FROM {} WHERE {};", table_name, constraints),
  )?
  .len()
    > 0;

  if already_present {
    // The entry already exists, update it
    let sets = fields
      .iter()
      .map(|f| {
        format!(
          "{}=\"{}\"",
          f,
          data.get(f).unwrap_or(&"undefined".to_string())
        )
      })
      .collect::<Vec<String>>()
      .join(",");
    let query = &format!("UPDATE {} SET {} WHERE {};", table_name, sets, constraints);
    connection.execute(query)?;
  } else {
    // No entry, create a new one
    let values: Vec<_> = fields
      .iter()
      .map(|x| data.get(x).unwrap_or(&"undefined".to_owned()).clone())
      .map(|x| format!("\"{}\"", x))
      .collect::<Vec<String>>();
    let column_names = fields.join(",");
    let query = &format!(
      "INSERT INTO {} ({}) VALUES ({});",
      table_name,
      column_names,
      values.join(",")
    );
    connection.execute(query)?;
  }
  Ok(())
}

pub struct SqlIndexStore {
  connection: Connection,
  table_name: String,
  fields: Vec<String>,
}

pub fn prepare_db(
  connection: &Connection,
  table_name: &str,
  fields: &Vec<String>,
) -> Result<(), Box<dyn Error>> {
  let table = fields
    .iter()
    .map(|s| s.to_string() + " TEXT NON NULL")
    .collect::<Vec<String>>()
    .join(",");
  connection.execute(&format!(
    "CREATE TABLE IF NOT EXISTS {} ({});",
    table_name, table
  ))?;
  Ok(())
}

impl SqlIndexStore {
  pub fn new(
    connection: Connection,
    table_name: &str,
    fields: Vec<String>,
  ) -> Result<Self, Box<dyn Error>> {
    prepare_db(&connection, table_name, &fields)?;
    Ok(SqlIndexStore {
      connection,
      table_name: String::from(table_name),
      fields,
    })
  }
}

impl IndexStore for SqlIndexStore {
  fn begin_transaction(&self) -> Result<(), Box<dyn Error>> {
    self.connection.execute("BEGIN TRANSACTION;")?;
    Ok(())
  }

  fn end_transaction(&self) -> Result<(), Box<dyn Error>> {
    self.connection.execute("END TRANSACTION;")?;
    Ok(())
  }

  fn write(self: &mut Self, data: &HashMap<String, String>) -> Result<(), Box<dyn Error>> {
    write_data(&self.connection, &self.table_name, &self.fields, data)
  }
}

#[derive(Clone)]
pub struct SqlIndexStoreWithMutex {
  connection: Arc<Mutex<Connection>>,
  table_name: String,
  fields: Vec<String>,
}

impl SqlIndexStoreWithMutex {
  pub fn new(
    connection: Connection,
    table_name: &str,
    mut fields: Vec<String>,
  ) -> Result<Self, Box<dyn Error>> {
    prepare_db(&connection, table_name, &mut fields)?;
    Ok(SqlIndexStoreWithMutex {
      connection: Arc::new(Mutex::new(connection)),
      table_name: String::from(table_name),
      fields,
    })
  }
}

impl IndexStore for SqlIndexStoreWithMutex {
  fn begin_transaction(&self) -> Result<(), Box<dyn Error>> {
    let connection = self.connection.lock().unwrap();
    connection.execute("BEGIN TRANSACTION;")?;
    Ok(())
  }

  fn end_transaction(&self) -> Result<(), Box<dyn Error>> {
    let connection = self.connection.lock().unwrap();
    connection.execute("END TRANSACTION;")?;
    Ok(())
  }

  fn write(self: &mut Self, data: &HashMap<String, String>) -> Result<(), Box<dyn Error>> {
    write_data(
      &self.connection.lock().unwrap(),
      &self.table_name,
      &self.fields,
      data,
    )
  }
}

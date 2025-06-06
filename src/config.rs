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

use serde::Deserialize;

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
  // Do we overwrite DICOM file on STORE
  pub store_overwrite: Option<bool>,
}

impl Config {
  pub fn get_indexable_fields(self: &Self) -> Vec<String> {
    self
      .indexing
      .fields
      .series
      .iter()
      .chain(
        self
          .indexing
          .fields
          .studies
          .iter()
          .chain(self.indexing.fields.instances.iter()),
      )
      .map(|s| s.clone())
      .collect::<Vec<String>>()
  }
}

use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub struct DicomError {
  details: String
}

impl DicomError {
  pub fn new(msg: &str) -> DicomError {
    DicomError{details: msg.to_string()}
  }
}

impl fmt::Display for DicomError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f,"{}",self.details)
  }
}

impl Error for DicomError {
  fn description(&self) -> &str {
    &self.details
  }
}

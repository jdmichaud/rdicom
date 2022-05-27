use std::fs;
use std::str::from_utf8;

pub fn has_dicom_header(buffer: &Vec<u8>) -> bool {
  from_utf8(&buffer[0x80..0x80 + 4]) == Ok("DICM")
}

pub fn is_dicom_file(file_path: &str) -> bool {
  match fs::read(file_path) {
    Ok(buf) => is_dicom(&buf),
    Err(_) => false,
  }
}

pub fn is_dicom(buffer: &Vec<u8>) -> bool {
  has_dicom_header(buffer)
}

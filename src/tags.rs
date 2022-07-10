#![allow(dead_code)]
#![allow(unused_variables)]

#[derive(Debug, PartialEq)]
pub struct Tag {
  pub group: u16,
  pub element: u16,
  pub name: &'static str,
  pub vr: &'static str,
  pub vm: std::ops::Range<u16>,
  pub description: &'static str,
}


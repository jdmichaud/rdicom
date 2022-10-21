#![allow(dead_code)]
#![allow(unused_variables)]

use core::hash;

#[derive(Debug, PartialEq, Eq)]
pub struct Tag {
  pub group: u16,
  pub element: u16,
  pub name: &'static str,
  pub vr: &'static str,
  pub vm: std::ops::Range<u16>,
  pub description: &'static str,
}

impl hash::Hash for Tag {
  fn hash<H: hash::Hasher>(&self, state: &mut H) {
      self.group.hash(state);
      self.element.hash(state);
  }
}

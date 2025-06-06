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

#![allow(dead_code)]
#![allow(unused_variables)]

use alloc::string::String;
use alloc::string::ToString;
use core::hash;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Tag {
  pub group: u16,
  pub element: u16,
  pub name: &'static str,
  pub vr: &'static str,
  pub vm: core::ops::Range<u16>,
  pub description: &'static str,
}

impl hash::Hash for Tag {
  fn hash<H: hash::Hasher>(&self, state: &mut H) {
    self.group.hash(state);
    self.element.hash(state);
  }
}

impl ToString for Tag {
  fn to_string(&self) -> String {
    format!("{:04x}{:04x}", self.group, self.element)
  }
}

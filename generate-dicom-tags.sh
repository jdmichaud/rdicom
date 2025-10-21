#!/bin/bash

# Copyright (c) 2023 Jean-Daniel Michaud
#
# Permission is hereby granted, free of charge, to any person obtaining a copy
# of this software and associated documentation files (the "Software"), to deal
# in the Software without restriction, including without limitation the rights
# to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
# copies of the Software, and to permit persons to whom the Software is
# furnished to do so, subject to the following conditions:
#
# The above copyright notice and this permission notice shall be included in all
# copies or substantial portions of the Software.
#
# THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
# IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
# FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
# AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
# LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
# OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
# SOFTWARE.

if [ $# -ne 1 ]
then
  echo "error: expected one parameter"
  echo "usage: $0 data-elements.csv"
  exit 1
fi

function isNumber() {
  local number=$1
  local re='^[0-9]+$'
  if [[ "$number" =~ $re ]] ; then
     return 1
  fi
  return 0
}

echo "please wait..." 1>&2

echo '// @generated'
echo '// Copyright (c) 2023 Jean-Daniel Michaud'
echo '//'
echo '// Permission is hereby granted, free of charge, to any person obtaining a copy'
echo '// of this software and associated documentation files (the "Software"), to deal'
echo '// in the Software without restriction, including without limitation the rights'
echo '// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell'
echo '// copies of the Software, and to permit persons to whom the Software is'
echo '// furnished to do so, subject to the following conditions:'
echo '//'
echo '// The above copyright notice and this permission notice shall be included in all'
echo '// copies or substantial portions of the Software.'
echo '//'
echo '// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR'
echo '// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,'
echo '// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE'
echo '// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER'
echo '// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,'
echo '// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE'
echo '// SOFTWARE.'
echo ''
echo '#![allow(dead_code)]'
echo '#![allow(unused_variables)]'
echo '#![allow(non_upper_case_globals)]'
echo ''
echo 'use alloc::string::String;'
echo 'use core::convert::TryFrom;'
echo ''
echo 'use crate::error::DicomError;'
echo 'use crate::tags::Tag;'
echo ''

cat $1 | \
while IFS=',' read -r -a array
do
  if [[ "${array[0]}" != *"x"* ]];
  then
    echo "pub const ${array[1]}: Tag = Tag {"
    echo "  group: 0x${array[0]:0:4},"
    echo "  element: 0x${array[0]:4:4},"
    echo "  name: \"${array[1]}\","
    # FIXME: In case of something like "US or SS" we only use the first VR for now
    echo "  vr: \"${array[2]:0:2}\","
    # TODO: convert ${array[3]} to a Range
    isNumber ${array[3]}
    res=$?
    if [[ $res -eq 1 ]]
    then
      echo "  vm: core::ops::Range { start: ${array[3]}, end: ${array[3]} },"
    else
      echo "  vm: core::ops::Range { start: 0, end: 0 },"
    fi
    echo "  description: \"${array[4]}\","
    echo "};"
    echo ""
  fi
done

echo "impl TryFrom<&str> for Tag {"
echo "  type Error = DicomError;"
echo ""
echo "  fn try_from(field_name: &str) -> Result<Self, Self::Error> {"
echo "    match field_name.to_uppercase().as_str() {"
cat $1 | \
while IFS=',' read -r -a array
do
  if [[ "${array[0]}" != *"x"* ]];
  then
    # to upper case
    echo "      \"${array[0]^^}\" => Ok(${array[1]}),"
    echo "      \"${array[1]^^}\" => Ok(${array[1]}),"
  fi
done
echo "      _ => Err(DicomError::new(&format!(\"Unknown field: {}\", field_name))),"
echo "    }"
echo "  }"
echo "}"
echo ""

echo "impl TryFrom<&String> for Tag {"
echo "  type Error = DicomError;"
echo ""
echo "  fn try_from(field_name: &String) -> Result<Self, Self::Error> {"
echo "    match field_name.to_uppercase().as_str() {"
cat $1 | \
while IFS=',' read -r -a array
do
  if [[ "${array[0]}" != *"x"* ]];
  then
    # to upper case
    echo "      \"${array[0]^^}\" => Ok(${array[1]}),"
    echo "      \"${array[1]^^}\" => Ok(${array[1]}),"
  fi
done
echo "      _ => Err(DicomError::new(&format!(\"Unknown field: {}\", field_name))),"
echo "    }"
echo "  }"
echo "}"
echo ""

echo "impl TryFrom<u32> for Tag {"
echo "  type Error = DicomError;"
echo ""
echo "  fn try_from(field: u32) -> Result<Self, Self::Error> {"
echo "    match field {"
cat $1 | \
while IFS=',' read -r -a array
do
  if [[ "${array[0]}" != *"x"* ]];
  then
    echo "      0x${array[0]} => Ok(${array[1]}),"
  fi
done
echo "      _ => Err(DicomError::new(&format!(\"Unknown tag: {:08x}\", field))),"
echo "    }"
echo "  }"
echo "}"
echo ""


echo "impl From<Tag> for String {"
echo "  fn from(tag: Tag) -> String {"
echo "    format!(\"{:0>4X}{:0>4X}\", tag.group, tag.element)"
echo "  }"
echo "}"
echo ""

#!/bin/bash

if [ $# -ne 1 ]
then
  echo "error: expected one parameter"
  echo "usage: $0 data-elements.csv"
  exit 1
fi

echo '#![allow(dead_code)]'
echo '#![allow(unused_variables)]'
echo '#![allow(non_upper_case_globals)]'
echo ''
echo 'use std::convert::TryFrom;'
echo ''
echo 'use crate::tags::Tag;'
echo 'use crate::error::DicomError;'
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
    echo "  vr: \"${array[2]}\","
    # TODO: convert ${array[3]} to a Range
    echo "  vm: std::ops::Range { start: 0, end: 0 },"
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
echo "      _ => Err(DicomError::new(&format!(\"Unknown tag: {}\", field))),"
echo "    }"
echo "  }"
echo "}"
echo ""

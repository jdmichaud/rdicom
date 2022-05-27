#!/bin/bash

if [ $# -ne 2 ]
then
  echo "error: expected one parameter"
  echo "usage: $0 data-elements.csv"
  exit 1
fi

echo '#![allow(dead_code)]'
echo '#![allow(unused_variables)]'
echo '#![allow(non_upper_case_globals)]'
echo ''
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
    echo "  vr: \"${array[2]}\","
    # TODO: convert ${array[3]} to a Range
    echo "  vm: std::ops::Range { start: 0, end: 0 },"
    echo "  description: \"${array[4]}\","
    echo "};"
    echo ""
  fi
done

echo "impl From<&str> for Tag {"
echo "  fn from(field_name: &str) -> Self {"
echo "    match field_name {"
cat $1 | \
while IFS=',' read -r -a array
do
  if [[ "${array[0]}" != *"x"* ]];
  then
    echo "      \"${array[1]}\" => ${array[1]},"
  fi
done
echo "      _ => unimplemented!(\"Unknown field: {}\", field_name),"
echo "    }"
echo "  }"
echo "}"
echo ""

echo "impl From<&String> for Tag {"
echo "  fn from(field_name: &String) -> Self {"
echo "    match field_name.as_str() {"
cat $1 | \
while IFS=',' read -r -a array
do
  if [[ "${array[0]}" != *"x"* ]];
  then
    echo "      \"${array[1]}\" => ${array[1]},"
  fi
done
echo "      _ => unimplemented!(\"Unknown field: {}\", field_name),"
echo "    }"
echo "  }"
echo "}"
echo ""

echo "impl From<u32> for Tag {"
echo "  fn from(field: u32) -> Self {"
echo "    match field {"
cat $1 | \
while IFS=',' read -r -a array
do
  if [[ "${array[0]}" != *"x"* ]];
  then
    echo "      0x${array[0]} => ${array[1]},"
  fi
done
echo "      _ => Tag {"
echo "        group: ((field & 0xFFFF0000) >> 16) as u16,"
echo "        element: (field & 0x0000FFFF) as u16,"
echo "        name: \"Unknown Tag & Data\","
echo "        vr: \"\","
echo "        vm: std::ops::Range { start: 0, end: 0 },"
echo "        description: \"Unknown Tag & Data\","
echo "      },"
echo "    }"
echo "  }"
echo "}"
echo ""

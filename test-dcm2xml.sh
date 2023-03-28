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

if [ $# -ne 2 ]
then
  echo "error: expected one parameter"
  echo "usage: $0 <path-to-binary> <path-to-dicom>"
  exit 1
fi

path_to_binary=$1
path_to_dicom=$2

shopt -s globstar
for filepath in ${path_to_dicom}/**/*; do # Whitespace-safe and recursive
  if dcm2xml --native-format "$filepath" > /tmp/output.xml 2> /dev/null; then
    if ${path_to_binary} -- /tmp/output.xml > /tmp/error.txt 2>&1; then
      echo "$filepath OK"
    else
      echo "$filepath KO"
      cat /tmp/error.txt
      # exit 1
    fi
  fi
done

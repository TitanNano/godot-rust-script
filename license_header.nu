#!/usr/bin/env nu

let source_root = $env.FILE_PWD

let license_notice = "/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */
" | lines

let license_length = $license_notice | length

def read_file [path: string]: nothing -> list<string> {
  open --raw $path | lines
}

def lines []: string -> list<string> {
  split row "\n"
}

def main []: nothing -> nothing {
  for file in (glob $"($source_root)/**/*.rs") {
    let current_header = read_file $file | first $license_length
   
    if $current_header == $license_notice {
      continue
    }

    read_file $file | prepend $license_notice | str join "\n" |  save -f $file.name
  }
}

#!/usr/bin/env nu

const GODOT_BUILDS = "godotengine/godot-builds"

let tmp_dir = mktemp -d
let godot_dev_dir = $"($tmp_dir)/godot_dev"
let godot_dev_zip = $"($godot_dev_dir).zip"

print -e $"fetching releases from ($GODOT_BUILDS)..."
let asset = http get $"https://api.github.com/repos/($GODOT_BUILDS)/releases"
  | filter {|item| $item.tag_name | str starts-with "4." }
  | get 0.assets
  | filter {|item| $item.name | str contains "linux.x86_64" }
  | get 0

print -e $"downloading prebuilt prerelease from ($asset.browser_download_url)..."
http get $asset.browser_download_url
  | save $godot_dev_zip

print -e "extracting zip archive..."
unzip -q -d $godot_dev_dir $godot_dev_zip

ls $godot_dev_dir | get 0.name | print

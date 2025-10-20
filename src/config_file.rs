// Copyright (c) 2025-2025 Jean-Daniel Michaud
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

use std::env;
use std::error::Error;
use std::path::Path;
use std::path::PathBuf;

pub enum ConfigProvenance {
  Default,
  XdgPath(String),
  CustomPath(String),
}

pub struct ConfigAccess {
  pub content: String,
  pub provenance: ConfigProvenance,
}

fn is_file_not_empty<P: AsRef<Path>>(path: P) -> bool {
  match std::fs::metadata(path) {
    Ok(metadata) => metadata.len() > 0,
    Err(..) => false,
  }
}

// Get the config file from the command line option --config
// Otherwise get it from XDG_CONFIG_PATH
// Otherwise use the default.
pub fn get_config(
  config_path: &Option<PathBuf>,
  default_config: &str,
) -> Result<ConfigAccess, Box<dyn Error>> {
  let mut provenance = ConfigProvenance::Default;
  // Load the config. We first check if a config file was provided as an option
  let content: String = if let Some(config_file) = &config_path {
    provenance = ConfigProvenance::CustomPath(config_file.to_string_lossy().to_string());
    // Try to load it
    match std::fs::read_to_string(&config_file) {
      Ok(config) => config,
      Err(e) => Err(format!("error: {e}: {}", config_file.display()))?,
    }
  } else {
    // Otherwise, try the standard path
    let default_config_file_path: String = env::var("XDG_CONFIG_HOME")
      .unwrap_or(env::var("HOME")? + "/.config/")
      + "/"
      + env!("CARGO_PKG_NAME")
      + "/config.yaml";

    if std::fs::metadata(std::path::Path::new(&default_config_file_path)).is_ok()
      && is_file_not_empty(&default_config_file_path)
    {
      provenance = ConfigProvenance::XdgPath(default_config_file_path.clone());
      // Try to load it
      match std::fs::read_to_string(&default_config_file_path) {
        Ok(config) => config,
        Err(e) => Err(format!("error: {e}: {}", default_config_file_path))?,
      }
    } else {
      // Otherwise, just use the embedded config file
      default_config.to_string()
    }
  };

  Ok(ConfigAccess {
    content,
    provenance,
  })
}

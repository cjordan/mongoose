// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::env;
use std::path::{Path, PathBuf};

fn bind_erfa(out_dir: &Path) {
    match pkg_config::probe_library("erfa") {
        Ok(lib) => {
            // Find erfa.h
            let mut erfa_include: Option<_> = None;
            for mut inc_path in lib.include_paths {
                inc_path.push("erfa.h");
                if inc_path.exists() {
                    erfa_include = Some(inc_path.to_str().unwrap().to_string());
                    break;
                }
            }

            bindgen::builder()
                .header(erfa_include.expect("Couldn't find erfa.h in pkg-config include dirs"))
                .whitelist_function("eraGmst06")
                .whitelist_function("eraGd2gc")
                .whitelist_var("ERFA_DJM0")
                .whitelist_var("ERFA_WGS84")
                .generate()
                .expect("Unable to generate bindings")
                .write_to_file(&out_dir.join("erfa.rs"))
                .expect("Couldn't write bindings");
        }
        Err(_) => panic!("Couldn't find the ERFA library via pkg-config"),
    };
}

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR env. variable not defined!"));
    bind_erfa(&out_dir);
}

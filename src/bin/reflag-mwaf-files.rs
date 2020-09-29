// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use anyhow::bail;
use structopt::StructOpt;

use mongoose::cotter::Occupancy;

/// Detect channels with high occupancy and flag them entirely. The input files
/// are named 1?????????_??.mwaf in the current directory, and the resulting
/// flags are written to RTS_1?????????_??.mwaf.
#[derive(StructOpt, Debug)]
#[structopt(name = "reflag-mwaf-files")]
struct Opts {
    /// The fraction of channels that must be flagged before we flag the entire
    /// channel. Must be between 0 and 1.
    #[structopt(short, long, default_value = "0.8")]
    threshold: f64,
}

fn main() -> Result<(), anyhow::Error> {
    let opts = Opts::from_args();
    // Ensure that the threshold is sensible.
    if opts.threshold == 0.0 {
        bail!("Not running with a threshold of 0.");
    } else if opts.threshold > 1.0 {
        bail!("The threshold cannot be bigger than 1.");
    }

    // Get all of the mwaf files.
    let mwaf_files = {
        let mut mwaf_files = vec![];
        let glob = globset::Glob::new("./1?????????_??.mwaf")?.compile_matcher();
        for entry in std::fs::read_dir(".")? {
            let entry = entry?.path();
            if glob.is_match(&entry) {
                mwaf_files.push(entry);
            }
        }
        mwaf_files
    };

    // Fail if there are no mwaf files.
    if mwaf_files.is_empty() {
        bail!("No files found matching: ./1?????????_??.mwaf");
    }

    // "Reflag" the mwaf file in a new file with "RTS_" as a prefix.
    for mwaf_file in mwaf_files {
        let occ = Occupancy::new(&mwaf_file)?;
        let rts_mwaf = format!("RTS_{}", mwaf_file.strip_prefix("./")?.display());
        occ.reflag(&rts_mwaf, opts.threshold as f64)?;
    }

    Ok(())
}

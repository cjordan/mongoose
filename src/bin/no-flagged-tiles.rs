// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::path::PathBuf;

use mwalib::mwalibContext;
use structopt::StructOpt;

/// Given a metafits file, print it out if it has no flagged tiles.
#[derive(StructOpt, Debug)]
#[structopt(name = "no-flagged-tiles")]
struct Opts {
    #[structopt(parse(from_str))]
    metafits: PathBuf,
}

fn main() -> Result<(), anyhow::Error> {
    let opts = Opts::from_args();

    let context = mwalibContext::new(&opts.metafits, &[])?;
    for rf in context.rf_inputs {
        if rf.flagged {
            std::process::exit(0);
        }
    }

    println!("{}", context.obsid);
    Ok(())
}

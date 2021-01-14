// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::convert::TryInto;
use std::ffi::CString;
use std::path::PathBuf;

use anyhow::{bail, ensure};
use fitsio::{errors::check_status as fits_check_status, FitsFile};
use itertools::Itertools;
use mwalib::mwalibContext;
use structopt::{clap::AppSettings, StructOpt};

/// MWA metafits files can list the delays of their tiles as all 32. This is
/// code for "this observation is bad". But if you really want to use it, this
/// tool will overwrite those delays with what is listed against the tiles (what
/// the observation actually used).
#[derive(StructOpt, Debug)]
#[structopt(name = "overwrite-metafits-delays", global_settings = &[AppSettings::ColoredHelp, AppSettings::ArgRequiredElseHelp])]
struct Opts {
    /// The path to the metafits file to be altered.
    #[structopt(parse(from_str))]
    metafits: PathBuf,

    /// Manually specify the delays to use. The default is to use the delays in
    /// the TILEDATA HDU.
    #[structopt(short, long)]
    delays: Option<Vec<u8>>,
}

fn main() -> anyhow::Result<()> {
    let opts = Opts::from_args();

    let delays: Vec<u8> = match opts.delays {
        None => {
            let context = mwalibContext::new(&opts.metafits, &[])?;
            // It's possible that a dipole is dead in the delays listed for a tile.
            // Iterate over all tiles until all values are non-32.
            let mut delays = vec![32; 16];
            for rf in context.rf_inputs {
                for (i, &d) in rf.delays.iter().enumerate() {
                    if d != 32 {
                        delays[i] = match d.try_into() {
                            Ok(n) => n,
                            Err(e) => bail!(
                                "Could not convert a delay ({}) in tile {} into a u8!\n{}",
                                d,
                                i,
                                e
                            ),
                        };
                    }
                }

                // Are all delays non-32?
                if delays.iter().all(|&d| d != 32) {
                    break;
                }
            }
            delays
        }

        Some(d) => {
            // Ensure 16 delays have been provided.
            ensure!(d.len() == 16, "When supplying delays, 16 must be given");
            d
        }
    };
    let delays_comma_str = delays.iter().join(",");

    // Modify the metafits file.
    {
        let mut meta = FitsFile::edit(&opts.metafits)?;
        meta.hdu(0)?;
        // Because hdu.write_key does not overwrite an existing key, use a
        // cfitsio-internal function to modify the key.
        let mut status = 0;
        let delays_cstr = CString::new("DELAYS").unwrap();
        let delays_comma_cstr = CString::new(delays_comma_str).unwrap();
        let preserve_comment_cstr = CString::new("&").unwrap();
        unsafe {
            fitsio_sys::ffmkys(
                meta.as_raw(),                  /* I - FITS file pointer  */
                delays_cstr.as_ptr(),           /* I - keyword name       */
                delays_comma_cstr.as_ptr(),     /* I - keyword value      */
                preserve_comment_cstr.as_ptr(), /* I - keyword comment    */
                &mut status,                    /* IO - error status      */
            );
        }
        fits_check_status(status)?;
    }

    Ok(())
}

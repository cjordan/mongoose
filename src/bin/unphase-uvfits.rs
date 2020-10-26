// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::f32::consts::TAU;
use std::fs::File;
use std::path::PathBuf;

use anyhow::bail;
use indicatif::{ProgressBar, ProgressStyle};
use num_complex::Complex32;
use structopt::StructOpt;

use fitsio::{errors::check_status as fits_check_status, FitsFile};

/// Convert the phase-tracked visibilities in a uvfits file to
/// non-phase-tracked, such that the RTS can read the visibilities correctly.
#[derive(StructOpt, Debug)]
#[structopt(name = "unphase-uvfits")]
struct Args {
    /// The uvfits file to be converted.
    #[structopt(name = "INPUT_UVFITS_FILE", parse(from_str))]
    input_uvfits: PathBuf,

    /// Alter the input uvfits file.
    #[structopt(long)]
    overwrite: bool,

    /// The path of the output uvfits file. Preserves the input uvfits file.
    #[structopt(long, parse(from_str))]
    output: Option<PathBuf>,
}

fn main() -> Result<(), anyhow::Error> {
    let args = Args::from_args();

    let mut uvfits = match (args.output, args.overwrite) {
        (None, false) => bail!("No output given, nor told to overwrite. Please specify one or the other!"),
        (Some(_), true) => bail!("An output was given, but I was also told to overwrite. Please specify one or the other!"),

        // Don't overwrite.
        (Some(pb), false) => {
            // Copy the input uvfits file to the output.
            let mut r = File::open(&args.input_uvfits)?;
            let mut w = File::create(&pb)?;
            std::io::copy(&mut r, &mut w)?;
            FitsFile::edit(&pb)?
        },

        // Overwrite.
        (None, true) => FitsFile::edit(args.input_uvfits)?
    };

    let hdu = uvfits.hdu(0)?;
    // GCOUNT tells us how many visibilities are in the file.
    let num_rows: u64 = hdu.read_key::<String>(&mut uvfits, "GCOUNT")?.parse()?;
    // PCOUNT tells us how many parameters are in each uvfits group.
    let pcount: usize = hdu.read_key::<String>(&mut uvfits, "PCOUNT")?.parse()?;
    // NAXIS2 is how many floats are associated with a cross pol (should be 3;
    // real part of visibilitiy, imag part of visibility, weight).
    let floats_per_pol: usize = hdu.read_key::<String>(&mut uvfits, "NAXIS2")?.parse()?;
    // NAXIS3 is the number of cross pols.
    let num_pols: usize = hdu.read_key::<String>(&mut uvfits, "NAXIS3")?.parse()?;

    // Get the frequency info.
    let num_fine_chan_freqs: usize = hdu.read_key::<String>(&mut uvfits, "NAXIS4")?.parse()?;
    let fine_chan_freqs: Vec<f32> = {
        let base_freq: f64 = hdu.read_key::<String>(&mut uvfits, "CRVAL4")?.parse()?;
        let base_index: isize = {
            // CRPIX might be a float. Parse it as one, then make it an int.
            let f: f64 = hdu.read_key::<String>(&mut uvfits, "CRPIX4")?.parse()?;
            f.round() as _
        };
        let fine_chan_width: f64 = hdu.read_key::<String>(&mut uvfits, "CDELT4")?.parse()?;

        let mut freqs = Vec::with_capacity(num_fine_chan_freqs);
        for i in 0..num_fine_chan_freqs {
            freqs.push((base_freq + (i as isize - base_index + 1) as f64 * fine_chan_width) as _);
        }

        // The RTS deviates from the uvfits standard with frequency. The RTS
        // expects everything to be half a fine channel width lower.
        update_double_key(&mut uvfits, "CRVAL4", base_freq - fine_chan_width / 2.0)?;

        freqs
    };

    // // TODO: Remove when Bart fixes the RTS.
    // // Swap the order of baseline and time. The RTS expects this particular
    // // order, but FHD does the opposite.
    // {
    //     update_string_key(&mut uvfits, "PTYPE4", "BASELINE")?;
    //     update_string_key(&mut uvfits, "PTYPE5", "DATE")?;

    //     let pzero4: f64 = hdu.read_key::<String>(&mut uvfits, "PZERO4")?.parse()?;
    //     let pzero5: f64 = hdu.read_key::<String>(&mut uvfits, "PZERO5")?.parse()?;
    //     update_double_key(&mut uvfits, "PZERO4", pzero5)?;
    //     update_double_key(&mut uvfits, "PZERO5", pzero4)?;

    //     let pscal4: f64 = hdu.read_key::<String>(&mut uvfits, "PSCAL4")?.parse()?;
    //     let pscal5: f64 = hdu.read_key::<String>(&mut uvfits, "PSCAL5")?.parse()?;
    //     update_double_key(&mut uvfits, "PSCAL4", pscal5)?;
    //     update_double_key(&mut uvfits, "PSCAL5", pscal4)?;
    // }

    // Correct the visibilities.
    let pb = ProgressBar::new(num_rows);
    pb.set_style(ProgressStyle::default_bar()
                 .template("{msg}{percent}% [{bar:34.cyan/blue}] {pos}/{len} rows [{elapsed_precise}<{eta_precise}]")
                 .progress_chars("#>-"));
    for row_num in 0..num_rows {
        let mut status = 0;
        // Read in the row's group parameters. We only read the first `pcount`
        // elements, but make the vector bigger for writing later.
        let mut group_params: Vec<f32> = vec![0.0; pcount];
        unsafe {
            fitsio_sys::ffggpe(
                uvfits.as_raw(),           /* I - FITS file pointer                       */
                1 + row_num as i64,        /* I - group to read (1 = 1st group)           */
                1,                         /* I - first vector element to read (1 = 1st)  */
                pcount as i64,             /* I - number of values to read                */
                group_params.as_mut_ptr(), /* O - array of values that are returned       */
                &mut status,               /* IO - error status                           */
            );
        }
        fits_check_status(status)?;

        // Read in the visibilities for this row.
        let mut vis: Vec<f32> = vec![0.0; num_fine_chan_freqs * floats_per_pol * num_pols];
        unsafe {
            fitsio_sys::ffgpve(
                uvfits.as_raw(),    /* I - FITS file pointer                       */
                1 + row_num as i64, /* I - group to read (1 = 1st group)           */
                1,                  /* I - first vector element to read (1 = 1st)  */
                vis.len() as i64,   /* I - number of values to read                */
                0.0,                /* I - value for undefined pixels              */
                vis.as_mut_ptr(),   /* O - array of values that are returned       */
                &mut 0,             /* O - set to 1 if any values are null; else 0 */
                &mut status,        /* IO - error status                           */
            );
        }
        fits_check_status(status)?;

        // // TODO: Remove when Bart fixes the RTS.
        // // Swap the order of baseline and time. The RTS expects this particular
        // // order, but FHD does the opposite.
        // {
        //     let date = group_params[3];
        //     let baseline = group_params[4];
        //     group_params[3] = baseline;
        //     group_params[4] = date;
        // }

        // Multiply the visibilities by e^(2 pi i w freq / c). Assume that w is
        // already divided by c.
        let w = group_params[2];
        for freq_index in 0..num_fine_chan_freqs {
            let (im, re) = (TAU * w * fine_chan_freqs[freq_index]).sin_cos();
            let c = Complex32::new(re, im);

            // These indices are for real XX, real YY, real XY, real YX. The
            // corresponding imag is one index ahead.
            for &i in &[0, 3, 6, 9] {
                let step = freq_index * floats_per_pol * num_pols + i;

                let mut v = Complex32::new(vis[step], vis[step + 1]);
                v *= c;
                // Put the real and imag parts back in the vec.
                vis[step] = v.re;
                vis[step + 1] = v.im;
            }
        }

        // Put the visibilities into `group_params` so we can do a single write.
        group_params.append(&mut vis);
        unsafe {
            fitsio_sys::ffpgpe(
                uvfits.as_raw(),           /* I - FITS file pointer                      */
                1 + row_num as i64,        /* I - group to write(1 = 1st group)          */
                1,                         /* I - first vector element to write(1 = 1st) */
                group_params.len() as i64, /* I - number of values to write              */
                group_params.as_mut_ptr(), /* I - array of values that are written       */
                &mut status,               /* IO - error status                          */
            );
        }
        fits_check_status(status)?;

        pb.set_position(row_num);
    }
    pb.finish();

    Ok(())
}

/// For some reason, when writing to keys that already exist with rust-fitsio,
/// the old one is not updated or removed. This function calls cfitsio directly
/// to get around that.
fn _update_string_key(fits: &mut FitsFile, key: &str, value: &str) -> Result<(), anyhow::Error> {
    let mut status = 0;
    let c_key = std::ffi::CString::new(key).unwrap();
    let c_value = std::ffi::CString::new(value).unwrap();
    unsafe {
        fitsio_sys::ffmkys(
            fits.as_raw(),    /* I - FITS file pointer  */
            c_key.as_ptr(),   /* I - keyword name       */
            c_value.as_ptr(), /* I - keyword value      */
            std::ptr::null(), /* I - keyword comment    */
            &mut status,      /* IO - error status      */
        );
    }
    fits_check_status(status)?;

    Ok(())
}

/// For some reason, when writing to keys that already exist with rust-fitsio,
/// the old one is not updated or removed. This function calls cfitsio directly
/// to get around that.
fn update_double_key(fits: &mut FitsFile, key: &str, value: f64) -> Result<(), anyhow::Error> {
    let mut status = 0;
    let c_key = std::ffi::CString::new(key).unwrap();
    unsafe {
        fitsio_sys::ffmkyd(
            fits.as_raw(),    /* I - FITS file pointer  */
            c_key.as_ptr(),   /* I - keyword name       */
            value,            /* I - keyword value      */
            10,               /* I - no of decimals     */
            std::ptr::null(), /* I - keyword comment    */
            &mut status,      /* IO - error status      */
        );
    }
    fits_check_status(status)?;

    Ok(())
}

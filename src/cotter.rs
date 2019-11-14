// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::path::{Path, PathBuf};

use fitsio::FitsFile;
use mwalib::*; // For fits-reading macros.

#[derive(Debug)]
pub struct Occupancy {
    /// The file that these statistics are derived from.
    pub mwaf_file: PathBuf,
    /// The number of times a specific frequency channel was flagged.
    pub flag_counts_per_channel: Vec<u32>,
    /// The fraction of which a specific frequency channel was flagged.
    pub flag_fraction_per_channel: Vec<f64>,
    /// The total number of samples. This can be used to work out the occupancy
    /// as a fraction.
    pub total_samples_per_channel: u32,
}

impl Occupancy {
    // TODO: Error handling.
    pub fn new<T: AsRef<Path>>(mwaf_file: &T) -> Result<Self, anyhow::Error> {
        let mut mwaf = fits_open!(&mwaf_file)?;
        let hdu = fits_open_hdu!(&mut mwaf, 0)?;
        let n_chans: usize = get_required_fits_key!(&mut mwaf, &hdu, "NCHANS")?;
        let n_antennas: usize = get_required_fits_key!(&mut mwaf, &hdu, "NANTENNA")?;
        let n_baselines = (n_antennas * (n_antennas + 1)) / 2;
        let n_scans: usize = get_required_fits_key!(&mut mwaf, &hdu, "NSCANS")?;

        // Get the flags out of the binary table. I think this is currently
        // bugged in the rust-fitsio crate, and I don't have the willpower to
        // investigate that further, so I'm calling cfitsio directly.
        let hdu = fits_open_hdu!(&mut mwaf, 1)?;
        let width: usize = get_required_fits_key!(&mut mwaf, &hdu, "NAXIS1")?;
        let flags = {
            let mut flags: Vec<u8> = vec![0; n_baselines * n_scans * width];
            let mut anynul: Vec<i32> = vec![0];
            let mut status: Vec<i32> = vec![0];
            unsafe {
                fitsio_sys::ffgcvb(
                    mwaf.as_raw(),       /* I - FITS file pointer                       */
                    1,                   /* I - number of column to read (1 = 1st col)  */
                    1,                   /* I - first row to read (1 = 1st row)         */
                    1,                   /* I - first vector element to read (1 = 1st)  */
                    flags.len() as i64,  /* I - number of values to read                */
                    0,                   /* I - value for null pixels                   */
                    flags.as_mut_ptr(),  /* O - array of values that are read           */
                    anynul.as_mut_ptr(), /* O - set to 1 if any values are null; else 0 */
                    status.as_mut_ptr(), /* IO - error status                           */
                );
            }
            flags
        };

        // Collapse the flags into a total number of flags per channel.
        let mut total: Vec<u32> = vec![0; n_chans];

        // Inspired by Brian Crosse. Add each unique byte to a "histogram" of
        // bytes, then unpack the bits from the bytes.
        let mut histogram: [u32; 256];
        for s in 0..width {
            histogram = [0; 256];
            for f in flags.iter().skip(s).step_by(width) {
                histogram[*f as usize] += 1;
            }
            // Unpack the histogram.
            for (v, h) in histogram.iter().enumerate() {
                for bit in 0..8 {
                    if ((v >> bit) & 0x01) == 0x01 {
                        total[7 * (s + 1) + s - bit] += h;
                    }
                }
            }
        }

        // Now normalise the totals, so they can be analysed as a fraction.
        let total_samples = (flags.len() / width) as u32;
        let occ_frac: Vec<f64> = total
            .iter()
            .map(|t| *t as f64 / total_samples as f64)
            .collect();

        Ok(Self {
            mwaf_file: mwaf_file.as_ref().canonicalize()?,
            flag_counts_per_channel: total,
            flag_fraction_per_channel: occ_frac,
            total_samples_per_channel: total_samples,
        })
    }

    // TODO: Error handling.
    /// Add header keys detailing which channels should be totally flagged.
    /// Derived from an old "reflag_mwaf_files.py" script.
    pub fn reflag<T: AsRef<Path>>(
        &self,
        new_mwaf_file: &T,
        threshold: f64,
    ) -> Result<(), anyhow::Error> {
        // Copy the original mwaf file to the new specified file.
        std::fs::copy(&self.mwaf_file, &new_mwaf_file)?;
        let mut rts_fits = FitsFile::edit(&new_mwaf_file)?;

        // For every occupancy exceeding the threshold, write a new header key.
        // I have no idea why it's done this way, but this is the way the old
        // python script did it...
        let mut n_reflag = 0;
        for (i, o) in self.flag_fraction_per_channel.iter().enumerate() {
            if *o > threshold {
                rts_fits.hdu(1)?.write_key(
                    &mut rts_fits,
                    &format!("REFLG_{:02}", n_reflag),
                    i as u32,
                )?;
                n_reflag += 1;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_1065880128() {
        // The mwaf file is zipped to save space in git. Unzip it to a temporary spot.
        let mut mwaf = tempfile::NamedTempFile::new().unwrap();
        let mut z =
            zip::ZipArchive::new(std::fs::File::open("tests/1065880128_01.mwaf.zip").unwrap())
                .unwrap();
        let mut z_mwaf = z.by_index(0).unwrap();
        std::io::copy(&mut z_mwaf, &mut mwaf).unwrap();

        let result = Occupancy::new(&mwaf);
        assert!(result.is_ok());
        let occ = result.unwrap();

        let expected = vec![
            1849343, 1849343, 155462, 152424, 150517, 149608, 149075, 149136, 149204, 149260,
            149317, 149354, 149279, 149515, 149632, 149908, 1849343, 149780, 149466, 149242,
            149163, 148877, 148873, 148811, 148693, 148713, 148771, 149406, 150996, 152602,
            1849343, 1849343,
        ];
        for (res, exp) in occ.flag_counts_per_channel.iter().zip(expected.iter()) {
            assert_eq!(res, exp);
        }

        // "Reflag" the mwaf file in a new temp file.
        let reflagged_mwaf = tempfile::NamedTempFile::new().unwrap();
        occ.reflag(&reflagged_mwaf, 0.8).unwrap();

        // Ensure that the edge and centre channels got picked up.
        let mut f = fits_open!(&reflagged_mwaf).unwrap();
        let hdu = fits_open_hdu!(&mut f, 1).unwrap();
        assert_eq!(0, get_required_fits_key!(&mut f, &hdu, "REFLG_00").unwrap());
        assert_eq!(1, get_required_fits_key!(&mut f, &hdu, "REFLG_01").unwrap());
        assert_eq!(
            16,
            get_required_fits_key!(&mut f, &hdu, "REFLG_02").unwrap()
        );
        assert_eq!(
            30,
            get_required_fits_key!(&mut f, &hdu, "REFLG_03").unwrap()
        );
        assert_eq!(
            31,
            get_required_fits_key!(&mut f, &hdu, "REFLG_04").unwrap()
        );
        // The REFLG_05 key shouldn't exist.
        let reflg_05: Result<u32, _> = get_required_fits_key!(&mut f, &hdu, "REFLG_05");
        assert!(reflg_05.is_err());
    }
}

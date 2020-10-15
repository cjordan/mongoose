// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::collections::BTreeSet;
use std::convert::TryInto;
use std::f32::consts::TAU;
use std::path::PathBuf;

use anyhow::bail;
use fitsio::FitsFile;
use indicatif::{ProgressBar, ProgressStyle};
use ndarray::{Array2, Axis};
use num_complex::Complex32;
use rayon::prelude::*;
use rubbl_casatables::{Table, TableOpenMode};
use structopt::StructOpt;

use mongoose::fits::uvfits::*;
use mongoose::ms::*;
use mongoose::VELC;

/// Convert an input measurement set to RTS-readable uvfits files.
#[derive(StructOpt, Debug)]
#[structopt(name = "ms-to-uvfits")]
struct Opts {
    /// The measurement set to be converted.
    #[structopt(name = "MEASUREMENT_SET", parse(from_str))]
    ms: PathBuf,

    /// Force this program to convert the input ms into a single uvfits file.
    /// The default is to make a uvfits file for each coarse band specified in
    /// the MWA_SUBBAND table.
    #[structopt(long)]
    one_to_one: bool,

    /// The stem of the uvfits files to be written, e.g. "/tmp/rts" will
    /// generate files named "/tmp/rts_band01.uvfits", "/tmp/rts_band02.uvfits",
    /// etc.
    ///
    /// If --one-to-one is specified, then this argument is the whole path to
    /// the output uvfits file (e.g. 1098108248.uvfits)
    #[structopt(short, long)]
    output: String,

    /// The name of the column containing the visibilities. This column should
    /// be in the main table of the measurement set.
    #[structopt(short, long, default_value = "OFFSET_DATA")]
    vis_col: String,

    /// Should this program undo phase tracking? The program assumes that the
    /// input visibilities are phase tracked, and setting this option will
    /// convert them to non-phase-tracked. The RTS expects non-phase-tracked
    /// visibilities.
    #[structopt(short, long)]
    undo_phase_tracking: bool,

    /// Should this program not carry the weights over from the measurement set?
    /// If we're resetting weights, all weights are set to 1.
    #[structopt(short, long)]
    reset_weights: bool,
}

fn main() -> Result<(), anyhow::Error> {
    let opts = Opts::from_args();

    // Open the main table of the input measurement set to get the number of
    // rows, as well as the start time of the observation.
    let (num_rows, start_epoch) = {
        let mut ms = Table::open(&opts.ms, TableOpenMode::Read).unwrap();
        let utc: f64 = ms.get_cell("TIME", 0).unwrap();
        (ms.n_rows(), casacore_utc_to_epoch(utc))
    };

    // `coarse_bands` contains MWA coarse band numbers, probably between 1 and
    // 24.
    let coarse_bands: Vec<u32> = if opts.one_to_one {
        vec![1]
    } else {
        // Get the coarse bands used in this MWA observation by looking at
        // the "MWA_SUBBAND" subtable. These start from 0, and we add 1 to
        // them.
        let mut t = Table::open(
            &format!("{}/MWA_SUBBAND", &opts.ms.display()),
            TableOpenMode::Read,
        )
        .unwrap();
        let coarse_bands_signed: Vec<i32> = t.get_col_as_vec("NUMBER").unwrap();
        let coarse_bands_unsigned: Result<Vec<_>, _> = coarse_bands_signed
            .into_iter()
            .map(|b| (b + 1).try_into())
            .collect();
        coarse_bands_unsigned?
    };

    // Get the channel information in the measurement set. Panic if there are 0
    // or 1 fine channels, and unless all widths are equal.
    let (total_bandwidth_hz, fine_chan_freqs_hz): (f64, Vec<f64>) = {
        let mut t = table_open(&opts.ms, Some("SPECTRAL_WINDOW"), TableOpenMode::Read);
        let widths: Vec<f64> = t.get_cell_as_vec("CHAN_WIDTH", 0).unwrap();
        if widths.len() <= 1 {
            bail!("Found {} fine channels; not continuing.", widths.len());
        }
        for w in &widths {
            if (*w - widths[0]).abs() > 1e-3 {
                bail!("Not all fine channel widths in the SPECTRAL_WINDOW table, CHAN_WIDTH column are equal!");
            }
        }

        let _centre_coarse_band: i32 = t.get_cell("MWA_CENTRE_SUBBAND_NR", 0).unwrap();
        let fine_chan_freqs_hz: Vec<f64> = t.get_cell_as_vec("CHAN_FREQ", 0).unwrap();
        // We assume that `total_bandwidth_hz` is the total bandwidth inside the
        // measurement set, not the whole observation.
        let total_bandwidth_hz: f64 = t.get_cell("TOTAL_BANDWIDTH", 0).unwrap();

        (total_bandwidth_hz, fine_chan_freqs_hz)
    };
    let coarse_chan_width_hz = total_bandwidth_hz / coarse_bands.len() as f64;
    let fine_chan_width_hz = fine_chan_freqs_hz[1] - fine_chan_freqs_hz[0];
    let fine_chans_per_coarse_band = fine_chan_freqs_hz.len() / coarse_bands.len();
    let _centre_freq_hz = fine_chan_freqs_hz[fine_chan_freqs_hz.len() / 2];

    let (ra_pointing_rad, dec_pointing_rad) = {
        let mut t = Table::open(
            &format!("{}/MWA_TILE_POINTING", &opts.ms.display()),
            TableOpenMode::Read,
        )
        .unwrap();
        let radec: Vec<f64> = t.get_cell_as_vec("DIRECTION", 0).unwrap();
        (radec[0], radec[1])
    };

    // Create and edit our output uvfits files. Because this is fairly slow,
    // we'll do it in parallel.
    let mut uvfits_filenames = Vec::with_capacity(coarse_bands.len());
    coarse_bands
        .par_iter()
        .map(|&band| {
            let centre_chan = (coarse_chan_width_hz / fine_chan_width_hz / 2.0).round() as u32;
            // The RTS expects this frequency...
            let centre_freq = fine_chan_freqs_hz[0]
                + (band - 1) as f64 * coarse_chan_width_hz
                + coarse_chan_width_hz / 2.0
                - fine_chan_width_hz / 2.0;

            let filename = if opts.one_to_one {
                (&opts.output).to_owned()
            } else {
                format!("{}_band{:02}.uvfits", &opts.output, band)
            };
            // Because we're working in parallel, and FitsFile structs can't be sent
            // over threads, ignore the returned structs. We'll just return the
            // filenames.
            let _ = new_uvfits(
                &filename,
                num_rows as i64,
                fine_chans_per_coarse_band as i64,
                &start_epoch,
                fine_chan_width_hz.round() as u32,
                centre_freq,
                centre_chan,
                ra_pointing_rad,
                dec_pointing_rad,
                None,
            )
            .expect("Failed to make a uvfits file.");
            filename
        })
        .collect_into_vec(&mut uvfits_filenames);
    let mut uvfits = Vec::with_capacity(coarse_bands.len());
    for f in uvfits_filenames {
        let u = FitsFile::edit(f)?;
        uvfits.push(u);
    }

    // Determine the number of time steps are in the measurement set.
    let n_time_steps = {
        // This is quite inefficient, but I don't see where this information is
        // listed!
        let mut ms = Table::open(&opts.ms, TableOpenMode::Read).unwrap();
        let mut time_set: BTreeSet<u64> = BTreeSet::new();
        let times: Vec<f64> = ms.get_col_as_vec("TIME").unwrap();
        for time in times {
            time_set.insert((time * 1e3).round() as _);
        }
        time_set.len() as u32
    };
    let n_baselines = (num_rows / n_time_steps as u64) as u32;
    // Quadratic equation. n^2 - n = n_baselines
    let _n_antennas = (1 + (((4 * n_baselines * 2) as f64).sqrt().round() as u32)) / 2;

    // Convert phase-tracked visibilities to non-phase-tracked visibilities, and
    // write them to the uvfits files.
    {
        // Open the main table of the input measurement set.
        let mut ms = Table::open(&opts.ms, TableOpenMode::Read).unwrap();

        // The truncated part of Julian Date. To calculate this, find the
        // "truncated" part of the JD by rounding it down to the nearest int,
        // and add 0.5
        let jd_trunc = start_epoch.as_jde_utc_days().floor() + 0.5;

        let blank_weights = Array2::<f32>::ones((fine_chan_freqs_hz.len(), 4));

        // Iterate over each row of the ms. For each ms row, we will write many
        // uvfits rows for each fine channel frequency.
        let mut row_num: u64 = 0;
        let pb = ProgressBar::new(num_rows);
        pb.set_style(
        ProgressStyle::default_bar()
            .template(
                "{msg}{percent}% [{bar:34.cyan/blue}] {pos}/{len} rows [{elapsed_precise}<{eta_precise}]",
            )
            .progress_chars("#>-"),
    );
        ms.for_each_row(|row| {
            // Get the uvw coordinates.
            let uvw: Vec<f64> = row.get_cell("UVW").unwrap();
            let u = (uvw[0] / VELC) as f32;
            let v = (uvw[1] / VELC) as f32;
            let w = (uvw[2] / VELC) as f32;

            // Get the uvfits baseline. It is encoded as a float, because all
            // elements of a uvfits row must have the same type.
            let ant1 = row.get_cell::<i32>("ANTENNA1").unwrap() + 1;
            let ant2 = row.get_cell::<i32>("ANTENNA2").unwrap() + 1;
            let baseline = encode_uvfits_baseline(ant1 as u32, ant2 as u32);

            // Get the jd_frac to put in this uvfits row. Subtract the true JD
            // by the truncated JD.
            let time: f64 = row.get_cell("TIME").unwrap();
            let jd = casacore_utc_to_epoch(time).as_jde_utc_days();
            let jd_frac = (jd - jd_trunc) as f32;

            // Convenient unit for the number of floats per uvfits file. There
            // are three per channel (real, imag and weight), and four per
            // channel (pol).
            let step = fine_chans_per_coarse_band * 4 * 3;

            // Get the visibilities out of the CORRECTED_DATA column. These are
            // the XX, XY, YX and YY visibilities for all frequency channels
            // (i.e. all coarse bands and all their fine channels). Assume that
            // weights are to be preserved.
            let uvfits_vis: Vec<f32> = {
                let mut vis: Array2<Complex32> = row.get_cell(&opts.vis_col).unwrap();
                let weights: Array2<f32> = row.get_cell("WEIGHT_SPECTRUM").unwrap();

                if opts.undo_phase_tracking {
                    // Multiply the visibilities by e^(2 pi i w freq / c) to
                    // undo the phase tracking (where i is the imaginary unit).
                    // `w` has already been divided by c above. Use de Moivre's
                    // theorem (means that we don't need to do a real complex
                    // exponential).
                    for (i, mut vis_chan) in vis.outer_iter_mut().enumerate() {
                        let (im, re) = (TAU * w * fine_chan_freqs_hz[i] as f32).sin_cos();
                        vis_chan *= Complex32::new(re, im);
                    }
                }
                // Isolate the instrumental polarisations from one another.
                let xx = vis.index_axis(Axis(1), 0);
                let xy = vis.index_axis(Axis(1), 1);
                let yx = vis.index_axis(Axis(1), 2);
                let yy = vis.index_axis(Axis(1), 3);
                let (wxx, wxy, wyx, wyy) = if opts.reset_weights {
                    (
                        blank_weights.index_axis(Axis(1), 0),
                        blank_weights.index_axis(Axis(1), 1),
                        blank_weights.index_axis(Axis(1), 2),
                        blank_weights.index_axis(Axis(1), 3),
                    )
                } else {
                    (
                        weights.index_axis(Axis(1), 0),
                        weights.index_axis(Axis(1), 1),
                        weights.index_axis(Axis(1), 2),
                        weights.index_axis(Axis(1), 3),
                    )
                };

                // Reinterpret the complex numbers as floats and stack the
                // visibilities in the order that uvfits expects (XX, YY, XY,
                // YX). Also put the weights in.
                let mut out = Vec::with_capacity(step);
                for (((((((c_xx, c_yy), c_xy), c_yx), w_xx), w_xy), w_yx), w_yy) in xx
                    .into_iter()
                    .zip(yy.into_iter())
                    .zip(xy.into_iter())
                    .zip(yx.into_iter())
                    .zip(wxx.into_iter())
                    .zip(wxy.into_iter())
                    .zip(wyx.into_iter())
                    .zip(wyy.into_iter())
                {
                    out.push(c_xx.re);
                    out.push(c_xx.im);
                    out.push(*w_xx);
                    out.push(c_yy.re);
                    out.push(c_yy.im);
                    out.push(*w_yy);
                    out.push(c_xy.re);
                    out.push(c_xy.im);
                    out.push(*w_xy);
                    out.push(c_yx.re);
                    out.push(c_yx.im);
                    out.push(*w_yx);
                }
                out
            };

            // Write the visibilities into the uvfits files.
            for (band, mut uvfits_file) in uvfits.iter_mut().enumerate() {
                // Make a new uvfits row for each uvfits file, because
                // `uvfits_vis` contains visibilities from all frequency bands.
                let mut uvfits_row = Vec::with_capacity(5 + step);
                uvfits_row.push(u);
                uvfits_row.push(v);
                uvfits_row.push(w);
                uvfits_row.push(baseline as f32);
                uvfits_row.push(jd_frac);
                for &vis in &uvfits_vis[(band * step)..((band + 1) * step)] {
                    uvfits_row.push(vis);
                }
                write_uvfits_vis(&mut uvfits_file, row_num as i64, uvfits_row)?;
            }

            row_num += 1;
            pb.set_position(row_num);
            Ok(())
        })
        .unwrap();
        pb.finish();
    };
    // Open the ANTENNA table of the input measurement set and fill the antenna
    // table of the uvfits files.
    {
        let pos =
            get_positions(&format!("{}/ANTENNA", &opts.ms.display()), Some("POSITION")).unwrap();
        let names =
            get_antenna_names(&format!("{}/ANTENNA", &opts.ms.display()), Some("NAME")).unwrap();
        for (band, mut u) in uvfits.iter_mut().enumerate() {
            let centre_freq = fine_chan_freqs_hz[0]
                + band as f64 * coarse_chan_width_hz
                + coarse_chan_width_hz / 2.0
                - fine_chan_width_hz / 2.0;
            write_uvfits_antenna_table(&mut u, &start_epoch, centre_freq, &names, pos.view())?;
        }
    }

    println!("Finished writing {} uvfits files.", uvfits.len());
    Ok(())
}

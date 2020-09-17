// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// TODO: Use QUACKTIM/GOODTIME for StartProcessingAt

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process::exit;

use mwalib::mwalibContext;
use structopt::StructOpt;

use mongoose::rts::*;

/// Generate a .in file suitable for RTS usage
#[derive(StructOpt, Debug)]
#[structopt(name = "rts-in-file-generator")]
enum Opts {
    /// Run the RTS in "patch" mode (direction-independent calibration)
    ///
    /// The base directory is expected to contain files from only a single
    /// observation, and have mwaf files that have been "re-flagged".
    Patch {
        #[structopt(flatten)]
        common: Common,
    },

    /// Run the RTS in "peel" mode (direction-dependent calibration)
    ///
    /// The base directory is expected to contain files from only a single
    /// observation, and have mwaf files that have been "re-flagged".
    Peel {
        /// The number of source calibrators to use.
        #[structopt(short, long)]
        num_cals: u32,

        /// The number of sources to peel. If not specified, defaults to
        /// num_cals.
        #[structopt(long)]
        num_peel: Option<u32>,

        #[structopt(flatten)]
        common: Common,
    },
}

/// Arguments that can be used in either the "patch" or "peel" modes of RTS
/// jobs.
#[derive(StructOpt, Debug)]
struct Common {
    /// The directory containing gpubox files and cotter mwaf files (formatted
    /// RTS_<obsid>_xy.mwaf).
    #[structopt(short, long, parse(from_os_str))]
    base_dir: PathBuf,

    /// The path to the obsid's metafits file.
    #[structopt(short, long, parse(from_os_str))]
    metafits: PathBuf,

    /// The path to the source list file.
    #[structopt(short, long, parse(from_os_str))]
    srclist: PathBuf,

    /// The number of integration bins to use. By default, this is determined by
    /// whether we're patching or peeling and the integration time of the
    /// observation.
    #[structopt(long)]
    num_integration_bins: Option<u32>,

    /// Use available cotter flags in mwaf file (doRFIflagging).
    #[structopt(short, long)]
    rfi_flagging: bool,

    /// Specify a sequence of integers corresponding to the coarse bands used
    /// (e.g. --subband-ids 1 2 3). Default is CHANSEL in the metafits file (but
    /// starting from 1, not 0).
    #[structopt(short = "S", long)]
    subband_ids: Option<Vec<u8>>,

    /// Use this to force the RA phase centre [degrees] (e.g. 10.3333).
    #[structopt(long)]
    force_ra: Option<f64>,

    /// Use this to force the Dec phase centre [degrees] (e.g. -27.0).
    #[structopt(long)]
    force_dec: Option<f64>,

    /// The number of channels to average during calibration (FscrunchChan).
    /// Default is 2.
    #[structopt(long)]
    fscrunch: Option<u8>,

    /// The number of "primary calibrators" to use. This should always be 1 for
    /// a patch. The default is 5 for a peel.
    #[structopt(long)]
    num_primary_cals: Option<u32>,

    /// Save the .in file to a specified location. If not specified, the .in
    /// file contents are printed to stdout.
    #[structopt(short, long)]
    output_file: Option<PathBuf>,
}

impl Opts {
    fn rts_params(self) -> RtsParams {
        let common = match &self {
            Self::Patch { common } => common,
            Self::Peel { common, .. } => common,
        };

        // Ideally, mwalib gets information from the gpubox files in addition to
        // the metafits file. But, we're not using any time information here, so
        // there's no need to handle gpubox files.
        let context = mwalibContext::new(&common.metafits, &[]).unwrap();

        let mode = match &self {
            Self::Patch { .. } => RtsMode::Patch,
            Self::Peel {
                num_cals, num_peel, ..
            } => RtsMode::Peel {
                num_cals: *num_cals,
                num_peel: if let Some(p) = num_peel {
                    // If num_peel was specified, use it.
                    *p
                } else {
                    // Otherwise, just use the specified `num_cals`.
                    *num_cals
                },
            },
        };

        let (
            time_resolution,
            corr_dumps_per_cadence_patch,
            num_integration_bins_patch,
            corr_dumps_per_cadence_peel,
            num_integration_bins_peel,
        ) = match context.integration_time_milliseconds {
            500 => (0.5, 128, 7, 16, 5),
            1000 => (1.0, 64, 7, 8, 3),
            2000 => (2.0, 32, 6, 4, 3),
            v => {
                eprintln!("Unhandled integration time: {}s", v as f64 / 1e3);
                exit(1)
            }
        };

        let (corr_dumps_per_cadence, mut num_integration_bins, num_iterations) = match mode {
            RtsMode::Patch => (corr_dumps_per_cadence_patch, num_integration_bins_patch, 1),
            RtsMode::Peel { .. } => (corr_dumps_per_cadence_peel, num_integration_bins_peel, 14),
        };
        if let Some(nib) = common.num_integration_bins {
            num_integration_bins = nib;
        }

        let num_fine_channels = match context.fine_channel_width_hz {
            40000 => 32,
            20000 => 64,
            10000 => 128,
            v => {
                eprintln!(
                    "Unhandled number of channels for fine-channel bandwidth {}kHz!",
                    v as f64 / 1e3
                );
                exit(1)
            }
        };

        // The magical base frequency is equal to:
        // (centre_freq - coarse_channel_bandwidth/2 - fine_channel_bandwidth/2)
        let base_freq = (context.metafits_centre_freq_hz
            - context.observation_bandwidth_hz / 2
            - context.fine_channel_width_hz / 2) as f64
            / 1e6;

        // Use the forced value, if provided.
        let obs_image_centre_ra = match common.force_ra {
            Some(r) => r,
            // Use RAPHASE if it is available.
            None => match context.ra_phase_center_degrees {
                Some(v) => v,
                // Otherwise, just use RA.
                None => context.ra_tile_pointing_degrees,
            },
        } / 15.0;

        let obs_image_centre_dec = match common.force_dec {
            Some(r) => r,
            None => match context.dec_phase_center_degrees {
                Some(v) => v,
                None => context.dec_tile_pointing_degrees,
            },
        };

        let subband_ids = match &common.subband_ids {
            Some(s) => s.clone(),
            None => context
                .coarse_channels
                .iter()
                .map(|cc| cc.gpubox_number as _)
                .collect(),
        };

        RtsParams {
            mode,
            base_dir: common.base_dir.clone(),
            metafits: common.metafits.clone(),
            source_catalogue_file: common.srclist.clone(),
            obsid: context.obsid,
            obs_image_centre_ra,
            obs_image_centre_dec,
            time_resolution,
            fine_channel_width_mhz: context.fine_channel_width_hz as f64 / 1e6,
            num_fine_channels,
            f_scrunch: common.fscrunch.unwrap_or(2),
            base_freq,
            subband_ids,
            num_primary_cals: common.num_primary_cals.unwrap_or(match &mode {
                RtsMode::Patch => 1,
                RtsMode::Peel { .. } => 5,
            }),
            do_rfi_flagging: common.rfi_flagging,
            corr_dumps_per_cadence,
            num_integration_bins,
            num_iterations,
        }
    }
}

fn main() -> Result<(), anyhow::Error> {
    let mut opts = Opts::from_args();
    let mut common = match &mut opts {
        Opts::Patch { common } => common,
        Opts::Peel { common, .. } => common,
    };

    // Sanity checks.
    // Test that the base directory exists, and make the path absolute.
    common.base_dir = common.base_dir.canonicalize().unwrap_or_else(|_| {
        eprintln!(
            "Specified base directory ({:?}) does not exist!",
            common.base_dir
        );
        exit(1)
    });

    // Test that the metafits file exists.
    if !common.metafits.exists() {
        eprintln!(
            "Specified metafits file ({:?}) does not exist!",
            common.metafits
        );
        exit(1)
    }

    // Test that the srclist file exists.
    if !common.srclist.exists() {
        eprintln!(
            "Specified source list file ({:?}) does not exist!",
            common.srclist
        );
        exit(1)
    };

    match &common.output_file {
        Some(f) => {
            let mut file = File::create(&f)?;
            write!(&mut file, "{}", opts.rts_params())?;
        }
        None => print!("{}", opts.rts_params()),
    }

    Ok(())
}

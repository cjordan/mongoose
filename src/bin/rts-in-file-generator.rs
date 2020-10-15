// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// TODO: Use QUACKTIM/GOODTIME for StartProcessingAt

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use anyhow::{bail, ensure};
use mwalib::mwalibContext;
use structopt::StructOpt;

use mongoose::rts::*;

#[derive(Debug, Default)]
struct Timing {
    corr_dump_time: f64,
    corr_dumps_per_cadence_patch: u32,
    num_integration_bins_patch: u32,
    corr_dumps_per_cadence_peel: u32,
    num_integration_bins_peel: u32,
}

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

        /// The number of "primary calibrators" to use (NumberOfCalibrators).
        /// This should always be 1 for a patch.
        #[structopt(long, default_value = "1")]
        num_primary_cals: u32,

        /// Specify the number of times to run the CML loop (NumberOfIterations).
        /// This should be 1 for a patch.
        #[structopt(long, default_value = "1")]
        num_iterations: u32,

        /// Write the visibilities processed by the RTS to uvfits files
        /// (writeVisToUVFITS).
        #[structopt(long)]
        write_vis_to_uvfits: bool,
    },

    /// Run the RTS in "peel" mode (direction-dependent calibration)
    ///
    /// The base directory is expected to contain files from only a single
    /// observation, and have mwaf files that have been "re-flagged".
    Peel {
        #[structopt(flatten)]
        common: Common,

        /// The number of source calibrators to use (NumberOfIonoCalibrators).
        #[structopt(short, long, default_value = "1000")]
        num_cals: u32,

        /// The number of sources to peel (NumberOfSourcesToPeel). If not
        /// specified, defaults to num_cals.
        #[structopt(long)]
        num_peel: Option<u32>,

        /// The number of "primary calibrators" to use (NumberOfCalibrators). If
        /// this is bigger than num-cals, then it will be truncated to match
        /// num-cals.
        #[structopt(long, default_value = "5")]
        num_primary_cals: u32,

        /// Specify the number of times to run the CML loop (NumberOfIterations).
        #[structopt(long, default_value = "14")]
        num_iterations: u32,

        /// Don't write the visibilities processed by the RTS to uvfits files
        /// (writeVisToUVFITS).
        #[structopt(long)]
        dont_write_vis_to_uvfits: bool,
    },
}

/// Arguments that can be used in either the "patch" or "peel" modes of RTS
/// jobs.
#[derive(StructOpt, Debug)]
struct Common {
    // File related.
    /// The directory containing input data, including gpubox files and cotter
    /// mwaf files (formatted RTS_<obsid>_xy.mwaf).
    #[structopt(short, long, parse(from_str))]
    base_dir: PathBuf,

    /// The base of the input filenames (BaseFilename). By default, this matches
    /// gpubox files.
    #[structopt(long, default_value = "*_gpubox")]
    base_filename: String,

    /// The path to the obsid's metafits file. If this isn't supplied, then many
    /// other variables must be supplied.
    #[structopt(short, long, parse(from_str))]
    metafits: Option<PathBuf>,

    /// The path to the source-list sky-model file.
    #[structopt(short, long, parse(from_str))]
    srclist: PathBuf,

    /// Don't use cotter flags in mwaf files (ImportCotterFlags). Default is to
    /// use flags.
    #[structopt(long)]
    no_cotter_flags: bool,

    /// Run the RTS's CheckForRFI routine (doRFIflagging).
    #[structopt(long)]
    rts_rfi_flagging: bool,

    /// Add a node number (01 to 24) to the base filename
    /// (AddNodeNumberToFilename). Used primarily (?) with input uvfits files.
    #[structopt(long)]
    add_node_number: bool,

    /// Don't correct visibilities for cable delays and PFB gains
    /// (doMWArxCorrections)
    #[structopt(long)]
    dont_rx_correct: bool,

    /// Don't apply cable corrections and digital gains based on metafits
    /// (doRawDataCorrections)
    #[structopt(long)]
    dont_correct_raw_data: bool,

    /// Don't read visibilities directly from gpubox files written by correlator
    /// (ReadGpuboxDirect)
    #[structopt(long)]
    dont_read_gpubox_direct: bool,

    /// When reading from uvfits, don't use a single file per coarse band
    /// (ReadAllFromSingleFile)
    #[structopt(long)]
    dont_read_all_from_single_file: bool,

    /// Use the 2016 FEE beam (TileBeamType=1).
    #[structopt(short = "f", long)]
    use_fee_beam: bool,

    /// The path to the FEE beam HDF5 file. If it's not specified, but
    /// --use-fee-beam is, inspect the MWA_BEAM_FILE environment variable.
    #[structopt(long)]
    fee_beam_file: Option<PathBuf>,

    // Observation related.
    /// The observation ID.
    #[structopt(long)]
    obsid: Option<u32>,

    /// Specify the time resolution of the input data in seconds (CorrDumpTime).
    /// The default is determined by the metafits file.
    #[structopt(long)]
    corr_dump_time: Option<f64>,

    /// Number of correlator dumps to be included in each calibration interval
    /// (CorrDumpsPerCadence).
    #[structopt(long)]
    corr_dumps_per_cadence: Option<u32>,

    /// The number of integration bins to use in baseline averaging
    /// (NumberOfIntegrationBins). By default, this is determined by whether
    /// we're patching or peeling and the integration time of the observation.
    ///
    /// The bins are set according to powers of two, so if
    /// CorrDumpsPerCadence=32, and NumberOfIntegrationBins=5, then the bins
    /// will have 32,16,8,4,2 visibilities.
    #[structopt(long)]
    num_integration_bins: Option<u32>,

    /// Frequency at middle of lowest fine channel in observation, in MHz
    /// (ObservationFrequencyBase). This is calculated by: obs_centre_freq -
    /// obs_bandwidth / 2 - fine_chan_width / 2
    #[structopt(long)]
    base_freq: Option<f64>,

    /// Specify the number of fine channels per coarse band (NumberOfChannels).
    #[structopt(long)]
    num_fine_chans: Option<u32>,

    /// Specify the fine channel bandwidth in MHz (ChannelBandwidth).
    #[structopt(long)]
    fine_chan_width: Option<f64>,

    /// Specify a sequence of integers corresponding to the coarse bands used
    /// (e.g. --subband-ids 1 2 3) (SubBandIDs). Default is CHANSEL in the
    /// metafits file (but starting from 1, not 0).
    #[structopt(short = "S", long)]
    subband_ids: Option<Vec<u8>>,

    /// Use this to force the RA phase centre in degrees (e.g. 10.3333)
    /// (ObservationImageCentreRA). Required if the metafits isn't given.
    #[structopt(long)]
    force_ra: Option<f64>,

    /// Use this to force the Dec phase centre in degrees (e.g. -27.0)
    /// (ObservationImageCentreDec). Required if the metafits isn't given.
    #[structopt(long)]
    force_dec: Option<f64>,

    /// The hour angle of the observation's pointing centre in decimal hours
    /// (e.g. 23.5) (ObservationPointCentreHA). Required if the metafits isn't
    /// given. Calculated with: LST - RA
    #[structopt(long)]
    ha_pointing_centre: Option<f64>,

    /// The declination of the observation's pointing centre as an hour angle in
    /// degrees (e.g. -27.0) (ObservationPointCentreDec). Required if the
    /// metafits isn't given.
    #[structopt(long)]
    dec_pointing_centre: Option<f64>,

    /// The number of channels to average during calibration (FscrunchChan).
    #[structopt(long, default_value = "2")]
    fscrunch: u8,

    /// By default, sourcelist vetoing removes sources from the sourcelist which
    /// are predicted to fall very close to the null of one of the coarse bands.
    /// Enabling this option turns vetoing off. (DisableSourcelistVetos)
    #[structopt(long)]
    disable_srclist_vetos: bool,

    /// Save the output of this program to a specified location. If not
    /// specified, the .in file contents are printed to stdout.
    #[structopt(short, long)]
    output_file: Option<PathBuf>,
}

impl Opts {
    fn rts_params(self) -> Result<RtsParams, anyhow::Error> {
        let common = match &self {
            Self::Patch { common, .. } => common,
            Self::Peel { common, .. } => common,
        };

        let fee_beam_file: Option<PathBuf> = if common.use_fee_beam {
            match &common.fee_beam_file {
                Some(f) => Some(f.clone()),
                // Try to get the file from MWA_BEAM_FILE
                None => match std::env::var("MWA_BEAM_FILE") {
                    Ok(f) => Some(PathBuf::from(f)),
                    Err(_) => bail!("--use-fee-beam was specified, but no --fee-beam-file was supplied, and couldn't access the MWA_BEAM_FILE variable."),
                },
            }
        } else {
            // We were told not to use the FEE beam, so there's no FEE beam
            // file.
            None
        };

        // mwalib gets accurate time information from the gpubox files in
        // addition to the metafits file (like what the true start time should
        // be, given that not all gpubox files start at the same time). But,
        // we're not using any time information here, so there's no need to
        // handle gpubox files.
        let context = if let Some(m) = &common.metafits {
            Some(mwalibContext::new(&m, &[])?)
        } else {
            None
        };

        let obsid = match common.obsid {
            Some(o) => o,
            None => match &context.as_ref().map(|c| c.obsid) {
                Some(o) => *o,
                None => {
                    bail!("Neither --obsid nor --metafits were specified; cannot get the obsid.")
                }
            },
        };

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

        // Set up the timing stuff. Fill things automatically first, if
        // possible, then overwrite settings with anything user-specified.
        let mut timing: Timing = match &context.as_ref().map(|c| c.integration_time_milliseconds) {
            Some(500) => Timing {
                corr_dump_time: 0.5,
                corr_dumps_per_cadence_patch: 128,
                num_integration_bins_patch: 7,
                corr_dumps_per_cadence_peel: 16,
                num_integration_bins_peel: 5,
            },
            Some(1000) => Timing {
                corr_dump_time: 1.0,
                corr_dumps_per_cadence_patch: 64,
                num_integration_bins_patch: 7,
                corr_dumps_per_cadence_peel: 8,
                num_integration_bins_peel: 3,
            },
            Some(2000) => Timing {
                corr_dump_time: 2.0,
                corr_dumps_per_cadence_patch: 32,
                num_integration_bins_patch: 6,
                corr_dumps_per_cadence_peel: 4,
                num_integration_bins_peel: 3,
            },
            _ => Timing::default(),
        };
        if let Some(c) = common.corr_dump_time {
            timing.corr_dump_time = c;
        }
        if let Some(c) = common.corr_dumps_per_cadence {
            timing.corr_dumps_per_cadence_patch = c;
            timing.corr_dumps_per_cadence_peel = c;
        }
        if let Some(c) = common.num_integration_bins {
            timing.num_integration_bins_patch = c;
            timing.num_integration_bins_peel = c;
        }

        // Check that all `timing` fields are non zero.
        if timing.corr_dump_time == 0.0
            || timing.corr_dumps_per_cadence_patch == 0
            || timing.corr_dumps_per_cadence_peel == 0
            || timing.num_integration_bins_patch == 0
            || timing.num_integration_bins_peel == 0
        {
            bail!("At least one of the timing fields was zero:\n{:?}\n\nIf you didn't specify any, it's possible that mongoose does not currently handle this integration time{}", timing, match context {
                Some(c) => format!(" ({}s)", c.integration_time_milliseconds as f64 / 1e3),
                None => "".to_string(),
            })
        }

        let (corr_dump_time, corr_dumps_per_cadence, num_integration_bins) = match mode {
            RtsMode::Patch => (
                timing.corr_dump_time,
                timing.corr_dumps_per_cadence_patch,
                timing.num_integration_bins_patch,
            ),
            RtsMode::Peel { .. } => (
                timing.corr_dump_time,
                timing.corr_dumps_per_cadence_peel,
                timing.num_integration_bins_peel,
            ),
        };

        let num_fine_channels = if let Some(n) = common.num_fine_chans {
            n
        } else {
            ensure!(context.is_some(), "Neither --num-fine-chans nor --metafits were specified; cannot get the number of fine channels.");
            let c = context.as_ref().unwrap();
            match c.fine_channel_width_hz {
                40000 => 32,  // 40 kHz
                20000 => 64,  // 20 kHz
                10000 => 128, // 10 kHz
                v => {
                    bail!(
                        "Unhandled number of channels for fine-channel bandwidth {}kHz!",
                        v as f64 / 1e3
                    );
                }
            }
        };

        // Could merge this with the block above, but that would be more effort
        // than I'm willing to expend right now.
        let fine_channel_width_mhz = if let Some(n) = common.fine_chan_width {
            n
        } else {
            ensure!(context.is_some(), "Neither --fine-chan-width nor --metafits were specified; cannot get the fine channel width.");
            let c = context.as_ref().unwrap();
            c.fine_channel_width_hz as f64 / 1e6
        };

        // The magical base frequency is equal to:
        // (centre_freq - coarse_channel_bandwidth/2 - fine_channel_bandwidth/2)
        let base_freq = if let Some(f) = common.base_freq {
            f
        } else {
            ensure!(
                context.is_some(),
                "Neither --base-freq nor --metafits were specified; cannot get the base frequency."
            );
            let c = context.as_ref().unwrap();
            (c.metafits_centre_freq_hz
                - c.observation_bandwidth_hz / 2
                - c.fine_channel_width_hz / 2) as f64
                / 1e6
        };

        // Use the forced value, if provided.
        let obs_image_centre_ra = match common.force_ra {
            Some(r) => r,
            // Use RAPHASE if it is available.
            None => {
                ensure!(
                    context.is_some(),
                    "Neither --force-ra nor --metafits were specified; cannot get the RA pointing."
                );
                let c = context.as_ref().unwrap();
                match c.ra_phase_center_degrees {
                    Some(v) => v,
                    // Otherwise, just use RA.
                    None => c.ra_tile_pointing_degrees,
                }
            }
        } / 15.0;

        let obs_image_centre_dec = match common.force_dec {
            Some(r) => r,
            None => {
                ensure!(
                    context.is_some(),
                    "Neither --force-dec nor --metafits were specified; cannot get the Dec pointing."
                );
                let c = context.as_ref().unwrap();
                match c.dec_phase_center_degrees {
                    Some(v) => v,
                    None => c.dec_tile_pointing_degrees,
                }
            }
        };

        let (obs_pointing_centre_ha, obs_pointing_centre_dec) = {
            let common = match &self {
                Opts::Patch { common, .. } => common,
                Opts::Peel { common, .. } => common,
            };
            match (
                &common.metafits,
                common.ha_pointing_centre,
                common.dec_pointing_centre,
            ) {
                // If we have a metafits, there's no need to populate these
                // fields. The RTS will get them from the metafits file.
                (Some(_), _, _) => (None, None),
                (None, Some(ha), Some(dec)) => (Some(ha), Some(dec)),
                // We need to bail if the pointing centre wasn't supplied when a
                // metafits also wasn't supplied.
                (None, _, _) => bail!("When not using a metafits file, both --ha-pointing-centre and --dec-pointing-centre must be specified.")
            }
        };

        let subband_ids = match &common.subband_ids {
            Some(s) => s.clone(),
            None => {
                ensure!(
                    context.is_some(),
                    "Neither --subband-ids nor --metafits were specified; cannot get the subbands."
                );
                let c = context.as_ref().unwrap();
                let mut cc: Vec<u8> = c
                    .coarse_channels
                    .iter()
                    .map(|cc| cc.gpubox_number as _)
                    .collect();
                cc.sort_unstable();
                cc
            }
        };

        Ok(RtsParams {
            mode,
            base_dir: common.base_dir.clone(),
            base_filename: common.base_filename.clone(),
            metafits: common.metafits.clone(),
            use_cotter_flags: !common.no_cotter_flags,
            source_catalogue_file: common.srclist.clone(),
            do_rfi_flagging: common.rts_rfi_flagging,
            do_rx_corrections: !common.dont_rx_correct,
            do_raw_data_corrections: !common.dont_correct_raw_data,
            read_gpubox_direct: !common.dont_read_gpubox_direct,
            read_all_from_single_file: !common.dont_read_all_from_single_file,
            add_node_number_to_filename: common.add_node_number,
            fee_beam_file,
            obsid,
            obs_image_centre_ra,
            obs_image_centre_dec,
            obs_pointing_centre_ha,
            obs_pointing_centre_dec,
            corr_dump_time,
            corr_dumps_per_cadence,
            num_integration_bins,
            num_iterations: match &self {
                Opts::Patch { num_iterations, .. } => *num_iterations,
                Opts::Peel { num_iterations, .. } => *num_iterations,
            },
            fine_channel_width_mhz,
            num_fine_channels,
            f_scrunch: common.fscrunch,
            base_freq,
            subband_ids,
            num_primary_cals: match &self {
                Opts::Patch {
                    num_primary_cals, ..
                } => *num_primary_cals,
                Opts::Peel {
                    num_cals,
                    num_primary_cals,
                    ..
                } => *num_primary_cals.min(num_cals),
            },
            disable_srclist_vetos: common.disable_srclist_vetos,
            write_vis_to_uvfits: match &self {
                Opts::Patch {
                    write_vis_to_uvfits,
                    ..
                } => *write_vis_to_uvfits,
                Opts::Peel {
                    dont_write_vis_to_uvfits,
                    ..
                } => !*dont_write_vis_to_uvfits,
            },
        })
    }
}

fn main() -> Result<(), anyhow::Error> {
    let mut opts = Opts::from_args();
    let mut common = match &mut opts {
        Opts::Patch { common, .. } => common,
        Opts::Peel { common, .. } => common,
    };

    // Sanity checks.
    // Test that the base directory exists, and make the path absolute.
    common.base_dir = match common.base_dir.canonicalize() {
        Ok(d) => d,
        Err(_) => bail!(
            "Specified base directory ({}) does not exist!",
            common.base_dir.display()
        ),
    };

    // Test that the metafits file exists.
    if let Some(m) = &common.metafits {
        ensure!(
            m.exists(),
            "Specified metafits file ({:?}) does not exist!",
            common.metafits
        );
    }

    // Test that the srclist file exists.
    ensure!(
        common.srclist.exists(),
        "Specified source list file ({:?}) does not exist!",
        common.srclist
    );

    match &common.output_file {
        Some(f) => {
            let mut file = File::create(&f)?;
            write!(&mut file, "{}", opts.rts_params()?)?;
        }
        None => print!("{}", opts.rts_params()?),
    }

    Ok(())
}

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::path::PathBuf;

use chrono::Utc;
use itertools::Itertools;
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref RE_GPUBOX_BAND: Regex = Regex::new(r"gpubox(0)?(?P<band>\d+)").unwrap();
}

#[derive(Clone, Copy, Debug)]
pub enum RtsMode {
    Patch,
    Peel {
        /// NumberOfIonoCalibrators: Number of sources to be used as Ionospheric
        /// Calibrators. Includes full DD calibrators, ie NumberOfCalibrators=1,
        /// NumberofIonoCalibrators=2 results in first source being given full
        /// DD cal, and second source calibrated for ionospheric offsets
        num_cals: u32,

        /// NumberOfSourcesToPeel: Number of sources to be subtracted from
        /// output images/visibilities.
        num_peel: u32,
    },
}

#[derive(Debug)]
pub struct RtsParams {
    /// The type of RTS processing we're doing.
    pub mode: RtsMode,

    // File related.
    /// The directory containing the observation's gpubox files (or uvfits
    /// files) and cotter flags (if required).
    pub base_dir: PathBuf,

    /// BaseFilename: String to match input visibility data. Typically written
    /// like "*_gpubox"
    pub base_filename: String,

    /// The metafits file associated with the observation (ReadMetafitsFile and
    /// MetafitsFilename).
    pub metafits: Option<PathBuf>,

    /// ImportCotterFlags: If true, ImportCotterBasename uses cotter mwaf files
    /// with a basename RTS_<obsid> in the `base_dir`.
    pub use_cotter_flags: bool,

    /// SourceCatalogueFile: Path to the sky model source list.
    pub source_catalogue_file: PathBuf,

    /// doRFIflagging: Apply internal RTS flagging. Flags whole fine channels.
    pub do_rfi_flagging: bool,

    /// doMWArxCorrections: Correct visibilities for cable delays and PFB gains.
    pub do_rx_corrections: bool,

    /// doRawDataCorrections: Apply cable corrections and digital gains based on metafits.
    pub do_raw_data_corrections: bool,

    /// ReadGpuboxDirect: Read visibilities directly from gpubox files written by correlator.
    pub read_gpubox_direct: bool,

    /// ReadAllFromSingleFile: Use a single file per coarse channel when reading
    /// from uvfits. Default is that each time/frequency interval is recorded in
    /// a separate uvfits file (as written by MAPS).
    pub read_all_from_single_file: bool,

    /// AddNodeNumberToFilename: Adds the mpi node number to the uvfits
    /// filename. Used when reading uvfits.
    pub add_node_number_to_filename: bool,

    /// The path to the FEE beam HDF5 file. If it's not specified, assume we're
    /// using the analytic beam.
    pub fee_beam_file: Option<PathBuf>,

    // Observation related.
    /// The observation ID. Should have 10 digits.
    ///
    /// This is only really needed to help the RTS identify cotter mwaf files.
    /// If you're not using cotter, then the obsid can be anything.
    pub obsid: u32,

    /// ObservationImageCentreRA: RA of image centre. Decimal hours.
    pub obs_image_centre_ra: f64,

    /// ObservationImageCentreDec: Declination of image centere. Decimal
    /// degrees.
    pub obs_image_centre_dec: f64,

    /// ObservationPointCentreHA: Hour Angle of Pointing Centre of Primary Beam.
    /// Decimal hours. Overridden by dipole delays if available.
    pub obs_pointing_centre_ha: Option<f64>,

    /// ObservationPointCentreDec: Declination of Pointing Centre of Primary
    /// Beam. Degrees. Overridden by dipole delays if available.
    pub obs_pointing_centre_dec: Option<f64>,

    /// CorrDumpTime: Output time resolution, or the cadence at which data is
    /// dumped in seconds
    pub corr_dump_time: f64,

    /// CorrDumpsPerCadence: Number of Correlator Dumps to be included in each
    /// calibration interval.
    pub corr_dumps_per_cadence: u32,

    /// NumberOfIntegrationBins: Number of bins to be used in baseline
    /// averaging. The bins are set according to powers of two, so if
    /// CorrDumpsPerCadence=32, and NumberOfIntegrationBins=5, then the bins
    /// will have 32,16,8,4,2 visibilities.
    pub num_integration_bins: u32,

    /// NumberOfIterations: Number of calibration intervals. For example, if
    /// this is 2 then the calibration and peeling loop will be run twice on two
    /// successive sets of CorrDumpTime * CorrDumpsPerCadence.
    pub num_iterations: u32,

    /// ChannelBandwidth: Bandwidth of fine channels in MHz
    pub fine_channel_width_mhz: f64,

    /// NumberOfChannels: The number of fine channels per coarse-band channel.
    pub num_fine_channels: u32,

    /// FscrunchChan: Number of fine channels to be averaged when creating
    /// output images/visibilities. ie 32 will create images which average over
    /// 32 fine channels.
    pub f_scrunch: u8,

    /// ObservationFrequencyBase: Frequency at middle of lowest fine channel in
    /// observation, in MHz, calculated with:
    ///
    /// centre_freq - coarse_channel_bandwidth/2 - fine_channel_bandwidth/2
    pub base_freq: f64,

    /// SubBandIDs: Which coarse-band channels to use during RTS processing.
    pub subband_ids: Vec<u8>,

    /// NumberOfCalibrators: The number of primary calibrators to use.
    pub num_primary_cals: u32,

    /// DisableSourcelistVetos: By default, sourcelist vetoing removes sources
    /// from the sourcelist which are predicted to fall very close to the null
    /// of one of the coarse bands. This can be turned off.
    pub disable_srclist_vetos: bool,

    /// writeVisToUVFITS: Write the visibilities processed by the RTS to uvfits
    /// files. The names are always uvdump_??.uvfits
    pub write_vis_to_uvfits: bool,
}

impl std::fmt::Display for RtsParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            r#"// RTS in file to {mode} obsid {obsid}
// Generated by mongoose v{version}
// at {time} UTC

FscrunchChan={fscrunch}

SubBandIDs={subband_ids}

StartProcessingAt=0

DoCalibration=1
generateDIjones={generate_di_jones}
useStoredCalibrationFiles={use_stored_calibration}
applyDIcalibration=1

BaseFilename={base_dir}/{base_filename}
{metafits}
{cotter}
doRFIflagging={rfi}
doMWArxCorrections={do_rx_corrections}
doRawDataCorrections={do_raw_data_corrections}
ReadGpuboxDirect={read_gpubox_direct}
ReadAllFromSingleFile={read_all_from_single_file}
AddNodeNumberToFilename={add_node_number_to_filename}
UsePacketInput=0
UseThreadedVI=0

{beam}

CorrDumpTime={time_res}
CorrDumpsPerCadence={cdpc}
NumberOfIntegrationBins={num_int_bins}
NumberOfIterations={num_ints}

ObservationFrequencyBase={base_freq}
NumberOfChannels={num_fine_channels}
ChannelBandwidth={fine_channel_bandwidth_mhz}

ObservationImageCentreRA={ra}
ObservationImageCentreDec={dec}
// Set these if delays are not in the metafits or there is no metafits.
{point_cent_ha}
{point_cent_dec}

SourceCatalogueFile={srclist}
NumberOfCalibrators={num_primary_cals}
{veto}
{write_uvfits}
{peel_only}

// Magic follows.
// MaxFrequency [MHz, float]: Used to set size of uv cells for gridding. Also
// affects binning of baselines by setting maximum decorrelation. Default is 300
// MHz.
MaxFrequency=200

// array_file.txt doesn't exist, but currently, the RTS will not run without it!
ArrayFile=array_file.txt
ArrayNumberOfStations=128

// Heaven help you if you're not using the MWA.
ArrayPositionLat=-26.70331940
ArrayPositionLong=116.67081524

calBaselineMin=20.0
calShortBaselineTaper=40.0

// ImageOversampling [float]: Sets oversampling of imaging pixel. Default value
// is 3.
ImageOversampling=3

// Store pixel beam weights along with intensity. Required if subsequently
// integrating images using integrate_image utility. Images will be 4X greater
// data volume.
StorePixelMatrices=0
"#,
            mode = match &self.mode {
                RtsMode::Patch => "patch",
                RtsMode::Peel { .. } => "peel",
            },
            obsid = format!("{}", self.obsid),
            version = env!("CARGO_PKG_VERSION"),
            time = Utc::now().format("%Y-%m-%d %H:%M:%S"),
            fscrunch = self.f_scrunch,
            subband_ids = self.subband_ids.iter().map(|x| format!("{}", x)).join(","),
            generate_di_jones = match &self.mode {
                RtsMode::Patch => 1,
                RtsMode::Peel { .. } => 0,
            },
            use_stored_calibration = match &self.mode {
                RtsMode::Patch => 0,
                RtsMode::Peel { .. } => 1,
            },
            metafits = match &self.metafits {
                Some(m) => format!(
                    "ReadMetafitsFile=1\n\
                     MetafitsFilename={}",
                    m.display()
                ),
                None => "ReadMetafitsFile=0".to_string(),
            },
            base_dir = self.base_dir.display(),
            base_filename = self.base_filename,
            cotter = if self.use_cotter_flags {
                format!(
                    "ImportCotterFlags=1\n\
                     ImportCotterBasename={}/RTS_{}",
                    self.base_dir.display(),
                    self.obsid
                )
            } else {
                "ImportCotterFlags=0".to_string()
            },
            rfi = if self.do_rfi_flagging { 1 } else { 0 },
            do_rx_corrections = if self.do_rx_corrections { 1 } else { 0 },
            do_raw_data_corrections = if self.do_raw_data_corrections { 1 } else { 0 },
            read_gpubox_direct = if self.read_gpubox_direct { 1 } else { 0 },
            read_all_from_single_file = if self.read_all_from_single_file { 1 } else { 0 },
            add_node_number_to_filename = if self.add_node_number_to_filename {
                1
            } else {
                0
            },
            beam = if let Some(f) = &self.fee_beam_file {
                format!(
                    "// FEE beam\n\
                     TileBeamType=1\n\
                     hdf5Filename={}",
                    f.display()
                )
            } else {
                "// Analytic beam\n\
                 useFastPrimaryBeamModels=1"
                    .to_string()
            },
            time_res = self.corr_dump_time,
            cdpc = self.corr_dumps_per_cadence,
            num_int_bins = self.num_integration_bins,
            num_ints = self.num_iterations,
            base_freq = self.base_freq,
            num_fine_channels = self.num_fine_channels,
            fine_channel_bandwidth_mhz = self.fine_channel_width_mhz,
            ra = self.obs_image_centre_ra,
            dec = self.obs_image_centre_dec,
            point_cent_ha = match self.obs_pointing_centre_ha {
                Some(ha) => format!("ObservationPointCentreHA={}", ha),
                None => "// ObservationPointCentreHA=".to_string(),
            },
            point_cent_dec = match self.obs_pointing_centre_dec {
                Some(dec) => format!("ObservationPointCentreDec={}", dec),
                None => "// ObservationPointCentreDec=".to_string(),
            },
            srclist = self
                .source_catalogue_file
                .to_str()
                .expect("Couldn't convert self.source_catalogue_file to string!"),
            num_primary_cals = self.num_primary_cals,
            veto = format!(
                "DisableSourcelistVetos={}",
                if self.disable_srclist_vetos { 1 } else { 0 }
            ),
            // Peel-only options.
            peel_only = match &self.mode {
                RtsMode::Patch => "".to_string(),
                RtsMode::Peel { num_cals, num_peel } => format!(
                    "UpdateCalibratorAmplitudes=1\n\
                     NumberOfIonoCalibrators={}\n\
                     NumberOfSourcesToPeel={}",
                    num_cals, num_peel
                ),
            },
            write_uvfits = format!(
                "writeVisToUVFITS={}",
                if self.write_vis_to_uvfits { 1 } else { 0 }
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rts_patch_output() {
        let obsid = 1000000000;
        let params = RtsParams {
            mode: RtsMode::Patch,
            base_dir: PathBuf::from("."),
            base_filename: "*_gpubox".to_string(),
            metafits: Some(PathBuf::from("cool.metafits")),
            use_cotter_flags: true,
            source_catalogue_file: PathBuf::from("cool_srclist.txt"),
            do_rfi_flagging: false,
            do_rx_corrections: true,
            do_raw_data_corrections: true,
            read_gpubox_direct: true,
            read_all_from_single_file: true,
            add_node_number_to_filename: false,
            fee_beam_file: None,
            obsid,
            obs_image_centre_ra: 0.0,
            obs_image_centre_dec: -27.0,
            obs_pointing_centre_ha: None,
            obs_pointing_centre_dec: None,
            corr_dump_time: 2.0,
            corr_dumps_per_cadence: 32,
            num_integration_bins: 7,
            num_iterations: 1,
            fine_channel_width_mhz: 0.04,
            num_fine_channels: 32,
            f_scrunch: 2,
            base_freq: 138.875,
            subband_ids: (1..=24).collect(),
            num_primary_cals: 1,
            disable_srclist_vetos: false,
            write_vis_to_uvfits: false,
        };
        let output = format!("{}", params);

        assert!(output.contains("RTS in file to patch"));
        assert!(output.contains("generateDIjones=1\n"));
        assert!(output.contains("useStoredCalibrationFiles=0\n"));
        assert!(output.contains("NumberOfCalibrators=1\n"));
        assert!(output.contains("writeVisToUVFITS=0\n"));
        assert!(!output.contains("UpdateCalibratorAmplitudes"));
        assert!(!output.contains("NumberOfIonoCalibrators"));
        assert!(!output.contains("NumberOfSourcesToPeel"));

        // These things aren't related to the patch mode, but should match
        // because of what was set up above.
        assert!(output.contains("ReadMetafitsFile=1\n"));
        assert!(output.contains("MetafitsFilename=cool.metafits\n"));
        assert!(output.contains("ImportCotterFlags=1\n"));
        assert!(output.contains(&format!("ImportCotterBasename=./RTS_{}\n", obsid)));
        assert!(output.contains("SourceCatalogueFile=cool_srclist.txt\n"));
        assert!(output.contains("useFastPrimaryBeamModels=1\n"));
        assert!(output.contains("DisableSourcelistVetos=0\n"));
        assert!(output.contains("doRFIflagging=0\n"));
        assert!(output.contains(
            "SubBandIDs=1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24\n"
        ));
    }

    #[test]
    fn test_rts_peel_output() {
        let obsid = 1000000000;
        let mode = RtsMode::Peel {
            num_cals: 1000,
            num_peel: 1000,
        };
        let params = RtsParams {
            mode,
            base_dir: PathBuf::from("."),
            base_filename: "*_gpubox".to_string(),
            metafits: Some(PathBuf::from("cool.metafits")),
            use_cotter_flags: false,
            source_catalogue_file: PathBuf::from("cool_srclist.txt"),
            do_rfi_flagging: true,
            do_rx_corrections: true,
            do_raw_data_corrections: true,
            read_gpubox_direct: true,
            read_all_from_single_file: true,
            add_node_number_to_filename: false,
            fee_beam_file: Some(PathBuf::from("/random/spot/beam_file.hdf5")),
            obsid,
            obs_image_centre_ra: 0.0,
            obs_image_centre_dec: -27.0,
            obs_pointing_centre_ha: None,
            obs_pointing_centre_dec: None,
            corr_dump_time: 2.0,
            corr_dumps_per_cadence: 32,
            num_integration_bins: 7,
            num_iterations: 1,
            fine_channel_width_mhz: 0.04,
            num_fine_channels: 32,
            f_scrunch: 2,
            base_freq: 138.875,
            subband_ids: (1..=3).collect(),
            num_primary_cals: 5,
            disable_srclist_vetos: true,
            write_vis_to_uvfits: true,
        };
        let output = format!("{}", params);

        assert!(output.contains("RTS in file to peel"));
        assert!(output.contains("generateDIjones=0\n"));
        assert!(output.contains("useStoredCalibrationFiles=1\n"));
        assert!(output.contains("NumberOfCalibrators=5\n"));
        assert!(output.contains("writeVisToUVFITS=1\n"));
        assert!(output.contains("UpdateCalibratorAmplitudes=1\n"));
        assert!(output.contains("NumberOfIonoCalibrators=1000\n"));
        assert!(output.contains("NumberOfSourcesToPeel=1000\n"));

        // These things aren't related to the peel mode, but should match
        // because of what was set up above.
        assert!(output.contains("ReadMetafitsFile=1\n"));
        assert!(output.contains("MetafitsFilename=cool.metafits\n"));
        assert!(output.contains("ImportCotterFlags=0\n"));
        assert!(!output.contains(&format!("ImportCotterBasename=./RTS_{}\n", obsid)));
        assert!(output.contains("SourceCatalogueFile=cool_srclist.txt\n"));
        assert!(!output.contains("useFastPrimaryBeamModels=1\n"));
        assert!(output.contains("TileBeamType=1\n"));
        assert!(output.contains("hdf5Filename=/random/spot/beam_file.hdf5\n"));
        assert!(output.contains("DisableSourcelistVetos=1\n"));
        assert!(output.contains("doRFIflagging=1\n"));
        assert!(output.contains("SubBandIDs=1,2,3\n"));
    }
}

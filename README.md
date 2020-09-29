# mongoose

Tools to help run the Murchison Widefield Array's Real-Time System (RTS)
software.

## Usage
### rts-in-file-generator
<details>

The majority of EoR observations can be calibrated by the RTS with the .in files
produced by:

``` sh
rts-in-file-generator patch \
                      --base-dir ".." \
                      --metafits "${METAFITS}" \
                      --srclist srclist_pumav3_*_patch*.txt \
                      -o rts_patch.in

rts-in-file-generator peel \
                      --base-dir ".." \
                      --metafits "${METAFITS}" \
                      --srclist srclist_pumav3_*_peel*.txt \
                      --num-cals 1000 \
                      --num-peel 1000 \
                      -o rts_peel.in
```

If you want to use the 2016 FEE beam, you should export the `MWA_BEAM_FILE`
environment variable with a path to its HDF5 file, e.g.:

    export MWA_BEAM_FILE=/pawsey/mwa/mwa_full_embedded_element_pattern.h5

then give `rts-in-file-generator` the `--use-fee-beam` flag (`-f`) for short.

A full sbatch script to set up RTS jobs appropriate for Pawsey's garrawarla
cluster follows. This assumes that you're submitting this script from a
directory *inside* a directory containing gpubox files and a metafits file, e.g.
2020-09-29_1307 below:

```
.
├── RTS_1065880128_??.mwaf
├── 1065880128_20131015134830_gpubox??_00.fits
├── 1065880128.metafits
├── 2020-09-29_1307
│   ├── rts_run.sh
│   ├── rts_setup.sh
```

<details>

``` sh
#!/bin/bash -l
#SBATCH --job-name=se_1098108248
#SBATCH --output=RTS-setup-1098108248-%A.out
#SBATCH --nodes=1
#SBATCH --ntasks-per-node=1
#SBATCH --time=00:05:00
#SBATCH --clusters=garrawarla
#SBATCH --partition=workq
#SBATCH --account=mwaeor
#SBATCH --export=NONE

module use /pawsey/mwa/software/python3/modulefiles
module load python-singularity
module load srclists/master
module load mongoose

# Will find one and only one metafits file in the parent directory.
METAFITS=$(find .. -maxdepth 1 -name "*.metafits" -print -quit)
[ $META ] && echo "No metafits file in current directory!" && exit 1

echo "Using ${METAFITS}"

set -eux

# Generate a source list for the patch step.
srclist_by_beam.py -n 1000 \
                   --srclist "${SRCLISTS_DIR}/srclist_pumav3_EoR0aegean_EoR1pietro+ForA.txt" \
                   --metafits "${METAFITS}"

# Generate a source list for the peel step.
srclist_by_beam.py -n 2000 \
                   --srclist "${SRCLISTS_DIR}/srclist_pumav3_EoR0aegean_EoR1pietro+ForA.txt" \
                   --metafits "${METAFITS}" \
                   --no_patch \
                   --cutoff=30

# Generate the RTS .in files for both patching and peeling.
rts-in-file-generator patch \
                      --base-dir ".." \
                      --metafits "${METAFITS}" \
                      --srclist srclist_pumav3_*_patch*.txt \
                      -o rts_patch.in

rts-in-file-generator peel \
                      --base-dir ".." \
                      --metafits "${METAFITS}" \
                      --srclist srclist_pumav3_*_peel*.txt \
                      --num-cals 1000 \
                      --num-peel 1000 \
                      -o rts_peel.in

# Ensure permissions are sensible!
find . -user $USER -type d -exec chmod g+rwx,o+rx,o-w {} \;
find . -user $USER -type f -exec chmod g+rw,o+r,o-w {} \;

echo "rts_setup.sh finished successfully."
```

</details>

</details>

### reflag-mwaf-files
<details>

Run `reflag-mwaf-files` in a directory containing .mwaf files (these should have
a filename structure `<obsid>_??.mwaf`, e.g. `1065880128_01.mwaf`). This will
produce "RTS mwaf" files, e.g. `RTS_1065880128_01.mwaf`. The RTS expects these
types of files to ingest cotter flags.

The point of this routine is to flag channels that have high RFI occupancy (by
default, >80%). This threshold can be tuned.

</details>

### ms-to-uvfits
<details>

Run `ms-to-uvfits` on the measurement set to be converted:

    ms-to-uvfits 1098108248.ms -o 1098108248

This will produce uvfits files named `1098108248_band01.uvfits`,
`1098108248_band02.uvfits`, etc.

You may need to specify `--vis-col` (`-v` for short) to tell the program which
visibilities to use. These are likely in the "DATA" column.

Also, the RTS expects the visibilities to not be phase tracked. Use
`--undo-phase-tracking` (`-u` for short) to convert phase-tracked visibilities
in the measurement set.

The following settings can be used to make .in files suitable for calibrating
uvfits files `1098108248_band??.uvfits`:

``` sh
rts-in-file-generator patch \
                      --base-dir "." \
                      --base-filename "1098108248_band" \
                      --srclist srclist_pumav3_*_patch*.txt \
                      --obsid 1098108248 \
                      --add-node-number \
                      --dont-correct-raw-data \
                      --dont-rx-correct \
                      --dont-read-gpubox-direct \
                      --no-cotter-flags \
                      --num-fine-chans 32 \
                      --fine-chan-width 0.04 \
                      --base-freq 138.875 \
                      --force-ra 0 \
                      --force-dec=-27 \
                      --ha-pointing-centre 0 \
                      --dec-pointing-centre=-27 \
                      --corr-dump-time 2 \
                      --corr-dumps-per-cadence 32 \
                      --num-integration-bins 6 \
                      --subband-ids 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 \
                      -o rts_patch.in

rts-in-file-generator peel \
                      --base-dir "." \
                      --base-filename "1098108248_band" \
                      --srclist srclist_pumav3_*_peel*.txt \
                      --obsid 1098108248 \
                      --add-node-number \
                      --dont-correct-raw-data \
                      --dont-rx-correct \
                      --dont-read-gpubox-direct \
                      --no-cotter-flags \
                      --num-fine-chans 32 \
                      --fine-chan-width 0.04 \
                      --base-freq 138.875 \
                      --force-ra 0 \
                      --force-dec=-27 \
                      --ha-pointing-centre 0 \
                      --dec-pointing-centre=-27 \
                      --corr-dump-time 2 \
                      --corr-dumps-per-cadence 4 \
                      --num-integration-bins 3 \
                      --num-cals 1000 \
                      --num-peel 1000 \
                      --subband-ids 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 \
                      -o rts_peel.in
```

</details>

## Installation
<details>

### Prerequisites

- A Rust compiler with a version >= 1.42.0

  `https://www.rust-lang.org/tools/install`

- [cfitsio](https://heasarc.gsfc.nasa.gov/docs/software/fitsio/)

- [erfa](https://github.com/liberfa/erfa)

- libclang

  This is a system library needed for some of `mongoose`'s dependencies.

  On Ubuntu, this library is provided by the package `libclang-dev`.

  On Arch, it is provided by the package `clang`.

### mongoose-specific instructions

- Compile the source

    `cargo build --release`

- Run a compiled binary

    `./target/release/rts-in-file-generator -h`

    A number of subcommands should present themselves, and the help text for
    each command should clarify usage.

    On the same system, the binaries can be copied and used anywhere you like!
</details>

## Troubleshooting

Report the version of the software used, your usage and the program output in a
new GitHub issue.

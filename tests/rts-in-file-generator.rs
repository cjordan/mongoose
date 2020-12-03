// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

/*!
 * This module tests the rts-in-file-generator command-line interface. It runs
 * the program with various arguments, hopefully to keep things sensible and
 * understood.
 */

#[cfg(test)]
mod tests {
    use assert_cmd::Command;

    fn cmd() -> Command {
        Command::cargo_bin("rts-in-file-generator").unwrap()
    }

    #[test]
    fn common_w_metafits() {
        // Test the most-frequently specified options, then check that
        // non-existant files fail.
        cmd()
            .arg("patch")
            .arg("--base-dir=..")
            .arg("--metafits=tests/1065880128.metafits")
            // Yeah, Cargo.toml isn't a source list, but we just need this to be
            // a real file. rts-in-file-generator doesn't verify that this file
            // is a real source list.
            .arg("--srclist=Cargo.toml")
            .assert()
            .success();

        // Base dir doesn't exist
        cmd()
            .arg("patch")
            .arg("--base-dir=/road/to/no/where")
            .arg("--metafits=tests/1065880128.metafits")
            .arg("--srclist=Cargo.toml")
            .assert()
            .failure();

        // Metafits doesn't exist
        cmd()
            .arg("patch")
            .arg("--base-dir=..")
            .arg("--metafits=tests/1065880128.metafits_asdf")
            .arg("--srclist=Cargo.toml")
            .assert()
            .failure();

        // Source list doesn't exist
        cmd()
            .arg("patch")
            .arg("--base-dir=..")
            .arg("--metafits=tests/1065880128.metafits")
            .arg("--srclist=ultimate-sky-model.txt")
            .assert()
            .failure();

        // Now test options without a metafits specified.
        cmd()
            .arg("patch")
            .arg("--base-dir=..")
            .arg("--srclist=Cargo.toml")
            .assert()
            .failure();
    }

    #[test]
    fn common_wo_metafits() {
        cmd()
            .arg("patch")
            .arg("--base-dir=..")
            .arg("--srclist=Cargo.toml")
            .assert()
            .failure();

        cmd()
            .arg("patch")
            .arg("--base-dir=..")
            .arg("--srclist=Cargo.toml")
            .arg("--obsid=1000000000")
            .assert()
            .failure();

        cmd()
            .arg("patch")
            .arg("--base-dir=..")
            .arg("--srclist=Cargo.toml")
            .arg("--obsid=1000000000")
            .arg("--corr-dump-time=2")
            .assert()
            .failure();

        cmd()
            .arg("patch")
            .arg("--base-dir=..")
            .arg("--srclist=Cargo.toml")
            .arg("--obsid=1000000000")
            .arg("--corr-dump-time=2")
            .arg("--corr-dumps-per-cadence=32")
            .assert()
            .failure();

        cmd()
            .arg("patch")
            .arg("--base-dir=..")
            .arg("--srclist=Cargo.toml")
            .arg("--obsid=1000000000")
            .arg("--corr-dump-time=2")
            .arg("--corr-dumps-per-cadence=32")
            .arg("--num-integration-bins=6")
            .assert()
            .failure();

        cmd()
            .arg("patch")
            .arg("--base-dir=..")
            .arg("--srclist=Cargo.toml")
            .arg("--obsid=1000000000")
            .arg("--corr-dump-time=2")
            .arg("--corr-dumps-per-cadence=32")
            .arg("--num-integration-bins=6")
            .arg("--num-fine-chans=32")
            .assert()
            .failure();

        cmd()
            .arg("patch")
            .arg("--base-dir=..")
            .arg("--srclist=Cargo.toml")
            .arg("--obsid=1000000000")
            .arg("--corr-dump-time=2")
            .arg("--corr-dumps-per-cadence=32")
            .arg("--num-integration-bins=6")
            .arg("--num-fine-chans=32")
            .arg("--fine-chan-width=0.04")
            .assert()
            .failure();

        cmd()
            .arg("patch")
            .arg("--base-dir=..")
            .arg("--srclist=Cargo.toml")
            .arg("--obsid=1000000000")
            .arg("--corr-dump-time=2")
            .arg("--corr-dumps-per-cadence=32")
            .arg("--num-integration-bins=6")
            .arg("--num-fine-chans=32")
            .arg("--fine-chan-width=0.04")
            .arg("--base-freq=150")
            .assert()
            .failure();

        cmd()
            .arg("patch")
            .arg("--base-dir=..")
            .arg("--srclist=Cargo.toml")
            .arg("--obsid=1000000000")
            .arg("--corr-dump-time=2")
            .arg("--corr-dumps-per-cadence=32")
            .arg("--num-integration-bins=6")
            .arg("--num-fine-chans=32")
            .arg("--fine-chan-width=0.04")
            .arg("--base-freq=150")
            .arg("--force-ra=0")
            .arg("--force-dec=-27")
            .assert()
            .failure();

        cmd()
            .arg("patch")
            .arg("--base-dir=..")
            .arg("--srclist=Cargo.toml")
            .arg("--obsid=1000000000")
            .arg("--corr-dump-time=2")
            .arg("--corr-dumps-per-cadence=32")
            .arg("--num-integration-bins=6")
            .arg("--num-fine-chans=32")
            .arg("--fine-chan-width=0.04")
            .arg("--base-freq=150")
            .arg("--force-ra=0")
            .arg("--force-dec=-27")
            .arg("--ha-pointing-centre=0")
            .arg("--dec-pointing-centre=-27")
            .assert()
            .failure();

        // Finally, success.
        cmd()
            .arg("patch")
            .arg("--base-dir=..")
            .arg("--srclist=Cargo.toml")
            .arg("--obsid=1000000000")
            .arg("--corr-dump-time=2")
            .arg("--corr-dumps-per-cadence=32")
            .arg("--num-integration-bins=6")
            .arg("--num-fine-chans=32")
            .arg("--fine-chan-width=0.04")
            .arg("--base-freq=150")
            .arg("--force-ra=0")
            .arg("--force-dec=-27")
            .arg("--ha-pointing-centre=0")
            .arg("--dec-pointing-centre=-27")
            .args(&["--subband-ids", "1", "2", "3"])
            .assert()
            .success();
    }

    #[test]
    fn fee_beam() {
        // No MWA_BEAM_FILE variable.
        cmd()
            .arg("patch")
            .arg("--base-dir=..")
            .arg("--metafits=tests/1065880128.metafits")
            .arg("--srclist=Cargo.toml")
            .arg("--use-fee-beam")
            .env_remove("MWA_BEAM_FILE")
            .assert()
            .failure();

        // File manually specified. The file doesn't get checked for validity.
        cmd()
            .arg("patch")
            .arg("--base-dir=..")
            .arg("--metafits=tests/1065880128.metafits")
            .arg("--srclist=Cargo.toml")
            .arg("--use-fee-beam")
            .arg("--fee-beam-file=Cargo.toml")
            .assert()
            .success();

        cmd()
            .arg("patch")
            .arg("--base-dir=..")
            .arg("--metafits=tests/1065880128.metafits")
            .arg("--srclist=Cargo.toml")
            .arg("--fee-beam-file=Cargo.toml")
            .assert()
            .success();

        // MWA_BEAM_FILE variable used.
        cmd()
            .arg("patch")
            .arg("--base-dir=..")
            .arg("--metafits=tests/1065880128.metafits")
            .arg("--srclist=Cargo.toml")
            .arg("--use-fee-beam")
            .env("MWA_BEAM_FILE", "Cargo.lock")
            .assert()
            .success();
    }
}

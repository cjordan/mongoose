// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

/*!
 * Helper fits functions.
 */

pub mod error;
pub mod uvfits;

use fitsio::{hdu::FitsHdu, FitsFile};
use hifitime::Epoch;

/// Use the DATESTRT key of an MWA metafits file to get a `hifitime::Epoch`
/// object. DATESTRT is assumed to be formatted like 2013-10-15T13:48:32
pub fn get_metafits_epoch(
    metafits: &mut FitsFile,
    hdu: &FitsHdu,
) -> Result<Epoch, fitsio::errors::Error> {
    // Get the Epoch from the date.
    let start_date: String = hdu.read_key(metafits, "DATESTRT")?;
    // Assume that the date is sensibly formatted, so I can be lazy and not do
    // proper error handling.
    let mut iter = start_date.split('T');
    let mut big_iter = iter.next().unwrap().split('-');
    let year = big_iter.next().unwrap().parse().unwrap();
    let month = big_iter.next().unwrap().parse().unwrap();
    let day = big_iter.next().unwrap().parse().unwrap();

    let mut small_iter = iter.next().unwrap().split(':');
    let hour = small_iter.next().unwrap().parse().unwrap();
    let minute = small_iter.next().unwrap().parse().unwrap();
    let second = small_iter.next().unwrap().parse().unwrap();

    let e = Epoch::from_gregorian_utc_hms(year, month, day, hour, minute, second);
    Ok(e)
}

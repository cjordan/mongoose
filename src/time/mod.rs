// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

/*!
 * Functions to help with time.
 */

use hifitime::Epoch;

/// From a `hifitime::Epoch`, get a formatted date string with the hours,
/// minutes and seconds set to 0.
///
/// e.g. "2013-10-15T00:00:00.0".
pub fn get_truncated_date_string(epoch: &Epoch) -> String {
    let (year, month, day, _, _, _, _) = epoch.as_gregorian_utc();
    format!(
        "{year}-{month}-{day}T00:00:00.0",
        year = year,
        month = month,
        day = day
    )
}

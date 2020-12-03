// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

/*!
 * Helper functions for measurement sets.
 */

use std::path::Path;

use hifitime::Epoch;
use ndarray::Array2;
use rubbl_casatables::{CasacoreError, Table, TableOpenMode};

/// Open a measurement set table. If `table` is `None`, then open the base
/// table.
pub fn table_open(ms: &Path, table: Option<&str>, open_mode: TableOpenMode) -> Table {
    Table::open(
        &format!("{}/{}", ms.display(), table.unwrap_or("")),
        open_mode,
    )
    .unwrap()
}

pub fn table_summary(ms: &mut Table) -> Result<(), CasacoreError> {
    println!(
        r#"n_rows: {n_rows},
n_columns: {n_columns},
column_names: {names:?},
table_keyword_names: {keywords:?}
"#,
        n_rows = ms.n_rows(),
        n_columns = ms.n_columns(),
        names = ms.column_names()?,
        keywords = ms.table_keyword_names()?
    );
    Ok(())
}

/// Get the antenna coordinates out of the supplied measurement set table. If
/// the column name isn't provided, assume it is "POSITION".
pub fn get_positions<T: AsRef<Path>>(
    table: &T,
    column_name: Option<&str>,
) -> Result<Array2<f64>, CasacoreError> {
    let col = column_name.unwrap_or("POSITION");

    let mut t = Table::open(table, TableOpenMode::Read).unwrap();
    let mut positions = Vec::with_capacity(3 * t.n_rows() as usize);
    t.for_each_row(|row| {
        let mut pos: Vec<f64> = row.get_cell(col).unwrap();
        positions.append(&mut pos);
        Ok(())
    })
    .unwrap();

    let arr = Array2::from_shape_vec((t.n_rows() as usize, 3), positions)
        .expect("ShapeError, shouldn't happen");
    Ok(arr)
}

/// The antenna names out of a measurement set table. If the column name isn't
/// provided, assume it is "NAME".
pub fn get_antenna_names<T: AsRef<Path>>(
    table: &T,
    column_name: Option<&str>,
) -> Result<Vec<String>, CasacoreError> {
    let col = column_name.unwrap_or("NAME");
    let mut t = Table::open(table, TableOpenMode::Read).unwrap();
    let names: Vec<String> = t.get_col_as_vec(col).unwrap();
    Ok(names)
}

/// Convert a casacore time to a `hifitime::Epoch`.
///
/// casacore uses seconds since 1858-11-17T00:00:00 (MJD epoch).
pub fn casacore_utc_to_epoch(utc_seconds: f64) -> Epoch {
    // The number of seconds between 1858-11-17T00:00:00 (MJD epoch, used by
    // casacore) and 1900-01-01T00:00:00 (TAI epoch) is 1297728000. I'm using
    // the TAI epoch because that's well supported by hifitime, and hifitime
    // converts an epoch to JD.
    let epoch_diff = 1297728000.0;

    // It appears that casacore does not count the number of leap seconds when
    // giving out the number of UTC seconds. This needs to be accounted for.
    // Because I don't have direct access to a table of leap seconds, and don't
    // want to constantly maintain one, I'm making a compromise; the method
    // below will be off by 1s if the supplied `utc_seconds` is near a leap
    // second.
    let num_leap_seconds = {
        let naive_obs_epoch = Epoch::from_tai_seconds(utc_seconds - epoch_diff);
        utc_seconds - epoch_diff - naive_obs_epoch.as_utc_seconds()
    };
    Epoch::from_tai_seconds(utc_seconds - epoch_diff + num_leap_seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_casacore_mjd_to_epoch() {
        // This MJD is taken from the 1065880128 observation.
        let mjd = 4888561712.0;
        let epoch = casacore_utc_to_epoch(mjd);
        assert_eq!(epoch.as_gregorian_utc_str(), "2013-10-15T13:48:32 UTC");
    }
}

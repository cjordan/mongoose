// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

/*!
 * Error handling for fits functions.
 */

use thiserror::Error;

#[derive(Error, Debug)]
pub enum FitsError {
    /// An error associated the fitsio-crate.
    #[error("{0}")]
    Fitsio(#[from] fitsio::errors::Error),

    /// An IO error.
    #[error("{0}")]
    IO(#[from] std::io::Error),
}

#[derive(Error, Debug)]
pub enum UvfitsError {
    /// An error associated with ERFA.
    #[error("{source_file}:{source_line} Call to ERFA function {function} returned status code {status}")]
    ERFA {
        source_file: String,
        source_line: u32,
        status: i32,
        function: String,
    },

    /// An error associated with fitsio.
    #[error("{0}")]
    Fitsio(#[from] fitsio::errors::Error),

    /// An error when converting a Rust string to a C string.
    #[error("{0}")]
    BadString(#[from] std::ffi::NulError),

    /// An IO error.
    #[error("{0}")]
    IO(#[from] std::io::Error),
}

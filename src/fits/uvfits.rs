// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

/*!
 * Functions specifically for uvfits files.
 */

use std::ffi::CString;
use std::path::PathBuf;

use fitsio::{errors::check_status as fits_check_status, FitsFile};
use hifitime::Epoch;
use ndarray::ArrayView2;

use super::error::*;
use crate::coords::XYZ;
use crate::erfa::{eraGd2gc, eraGmst06, ERFA_DJM0, ERFA_WGS84};
use crate::time::*;

/// Helper function to convert strings into pointers to C strings.
///
/// This is currently intended for use only with MWA tile names (e.g. Tile104),
/// and due to a bug in rubbl_casatables, the first string pulled out of a
/// measurement set's column is always null; this function assumes that null
/// string is Tile011.
fn rust_strings_to_c_strings(strings: &[String]) -> Result<Vec<*mut i8>, std::ffi::NulError> {
    let mut c_strings = Vec::with_capacity(strings.len());
    for s in strings {
        // let c_str = CString::new(s.as_str())?;

        // If CString::new fails, assume this is the first tile name, and assume
        // the correct tile name. Hopefully this is a fixable bug.
        let c_str = CString::new(s.as_str()).unwrap_or_else(|_| CString::new("Tile011").unwrap());
        c_strings.push(c_str.into_raw());
    }
    Ok(c_strings)
}

/// Encode a baseline into the uvfits format. Use the miriad convention to
/// handle more than 255 antennas (up to 2048). This is backwards compatible
/// with the standard UVFITS convention. Antenna indices start at 1.
///
/// Shamelessly copied from the RTS, originally written by Randall Wayth.
pub fn encode_uvfits_baseline(b1: u32, b2: u32) -> u32 {
    if b2 > 255 {
        b1 * 2048 + b2 + 65536
    } else {
        b1 * 256 + b2
    }
}

/// Create a new uvfits file at the specified location.
///
/// This function makes no assumptions, and hence cannot use mwalib.
pub fn new_uvfits(
    filename: &str,
    num_rows: i64,
    num_chans: i64,
    start_epoch: &Epoch,
    fine_chan_width_hz: u32,
    centre_freq_hz: f64,
    centre_freq_chan: u32,
    ra_rad: f64,
    dec_rad: f64,
    obs_name: Option<&str>,
) -> Result<FitsFile, UvfitsError> {
    // Delete any file that already exists.
    if PathBuf::from(filename).exists() {
        std::fs::remove_file(&filename)?;
    }

    // Create a new fits file.
    let mut status = 0;
    let c_filename = CString::new(filename)?;
    let mut fptr = std::ptr::null_mut();
    unsafe {
        fitsio_sys::ffinit(
            &mut fptr as *mut *mut _, /* O - FITS file pointer                   */
            c_filename.as_ptr(),      /* I - name of file to create              */
            &mut status,              /* IO - error status                       */
        );
    }
    fits_check_status(status)?;

    // Initialise the group header. Copied from cotter. -32 means FLOAT_IMG.
    let naxis = 6;
    let mut naxes = [0, 3, 4, num_chans as i64, 1, 1];
    let num_group_params = 5;
    unsafe {
        fitsio_sys::ffphpr(
            fptr,               /* I - FITS file pointer                        */
            1,                  /* I - does file conform to FITS standard? 1/0  */
            -32,                /* I - number of bits per data value pixel      */
            naxis,              /* I - number of axes in the data array         */
            naxes.as_mut_ptr(), /* I - length of each data axis                 */
            num_group_params,   /* I - number of group parameters (usually 0)   */
            num_rows,           /* I - number of random groups (usually 1 or 0) */
            1,                  /* I - may FITS file have extensions?           */
            &mut status,        /* IO - error status                            */
        );
    }
    fits_check_status(status)?;

    // Finally close the file.
    unsafe {
        fitsio_sys::ffclos(fptr, &mut status);
    }
    fits_check_status(status)?;

    // Open the fits file with rust-fitsio.
    let mut u = FitsFile::edit(filename)?;
    let hdu = u.hdu(0)?;
    hdu.write_key(&mut u, "BSCALE", 1.0)?;

    // Set header names and scales.
    for (i, &param) in ["UU", "VV", "WW", "BASELINE", "DATE"].iter().enumerate() {
        hdu.write_key(&mut u, &format!("PTYPE{}", i + 1), param)?;
        hdu.write_key(&mut u, &format!("PSCAL{}", i + 1), 1.0)?;
        if i != 4 {
            hdu.write_key(&mut u, &format!("PZERO{}", i + 1), 0.0)?;
        } else {
            // Set the zero level for the DATE column.
            hdu.write_key(
                &mut u,
                "PZERO5",
                start_epoch.as_jde_utc_days().floor() + 0.5,
            )?;
        }
    }
    hdu.write_key(&mut u, "DATE-OBS", get_truncated_date_string(&start_epoch))?;

    // Dimensions.
    hdu.write_key(&mut u, "CTYPE2", "COMPLEX")?;
    hdu.write_key(&mut u, "CRVAL2", 1.0)?;
    hdu.write_key(&mut u, "CRPIX2", 1.0)?;
    hdu.write_key(&mut u, "CDELT2", 1.0)?;

    // Linearly polarised.
    hdu.write_key(&mut u, "CTYPE3", "STOKES")?;
    hdu.write_key(&mut u, "CRVAL3", -5)?;
    hdu.write_key(&mut u, "CDELT3", -1)?;
    hdu.write_key(&mut u, "CRPIX3", 1.0)?;

    hdu.write_key(&mut u, "CTYPE4", "FREQ")?;
    hdu.write_key(&mut u, "CRVAL4", centre_freq_hz)?;
    hdu.write_key(&mut u, "CDELT4", fine_chan_width_hz)?;
    hdu.write_key(&mut u, "CRPIX4", centre_freq_chan + 1)?;

    hdu.write_key(&mut u, "CTYPE5", "RA")?;
    hdu.write_key(&mut u, "CRVAL5", ra_rad.to_degrees())?;
    hdu.write_key(&mut u, "CDELT5", 1)?;
    hdu.write_key(&mut u, "CRPIX5", 1)?;

    hdu.write_key(&mut u, "CTYPE6", "DEC")?;
    hdu.write_key(&mut u, "CRVAL6", dec_rad.to_degrees())?;
    hdu.write_key(&mut u, "CDELT6", 1)?;
    hdu.write_key(&mut u, "CRPIX6", 1)?;

    hdu.write_key(&mut u, "OBSRA", ra_rad.to_degrees())?;
    hdu.write_key(&mut u, "OBSDEC", dec_rad.to_degrees())?;
    hdu.write_key(&mut u, "EPOCH", 2000.0)?;

    hdu.write_key(&mut u, "OBJECT", obs_name.unwrap_or("Undefined"))?;
    hdu.write_key(&mut u, "TELESCOP", "MWA")?;
    hdu.write_key(&mut u, "INSTRUME", "MWA")?;

    // This is apparently required...
    let history = CString::new("AIPS WTSCAL =  1.0").unwrap();
    unsafe {
        fitsio_sys::ffphis(
            u.as_raw(),       /* I - FITS file pointer  */
            history.as_ptr(), /* I - history string     */
            &mut status,      /* IO - error status      */
        );
    }
    fits_check_status(status)?;

    // Add in version information
    let comment = CString::new(format!(
        "Created by {} v{}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    ))
    .unwrap();
    unsafe {
        fitsio_sys::ffpcom(
            u.as_raw(),       /* I - FITS file pointer   */
            comment.as_ptr(), /* I - comment string      */
            &mut status,      /* IO - error status       */
        );
    }
    fits_check_status(status)?;

    hdu.write_key(&mut u, "SOFTWARE", env!("CARGO_PKG_NAME"))?;
    hdu.write_key(
        &mut u,
        "GITLABEL",
        format!("v{}", env!("CARGO_PKG_VERSION")),
    )?;

    Ok(u)
}

/// Write the antenna table to a uvfits file.
///
/// `start_epoch` is a `hifitime::Epoch` struct derived from the first time
/// going into the uvfits file. `centre_freq` is the centre frequency of the
/// coarse band that this uvfits file pertains to. `positions` are the absolute
/// XYZ coordinates of the MWA tiles. These positions need to have the MWA's
/// "centre" XYZ coordinates subtracted to make them local XYZ.
///
/// `uvfits` must have been opened in write mode, and should only have a single
/// HDU when this function is called.
// Derived from cotter.
pub fn write_uvfits_antenna_table(
    uvfits: &mut FitsFile,
    start_epoch: &Epoch,
    centre_freq: f64,
    antenna_names: &[String],
    positions: ArrayView2<f64>,
) -> Result<(), UvfitsError> {
    // Stuff that a uvfits file always expects?
    let col_names: Vec<String> = vec![
        "ANNAME", "STABXYZ", "NOSTA", "MNTSTA", "STAXOF", "POLTYA", "POLAA", "POLCALA", "POLTYB",
        "POLAB", "POLCALB",
    ]
    .iter()
    .map(|&s| s.to_string())
    .collect();
    let col_formats: Vec<String> = vec![
        "8A", "3D", "1J", "1J", "1E", "1A", "1E", "3E", "1A", "1E", "3E",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let col_units: Vec<String> = vec![
        "", "METERS", "", "", "METERS", "", "DEGREES", "", "", "DEGREES", "",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let mut c_col_names = rust_strings_to_c_strings(&col_names)?;
    let mut c_col_formats = rust_strings_to_c_strings(&col_formats)?;
    let mut c_col_units = rust_strings_to_c_strings(&col_units)?;
    let extname = CString::new("AIPS AN").unwrap();

    // ffcrtb creates a new binary table in a new HDU. This should be the second
    // HDU, so there should only be one HDU before this function is called.
    let mut status = 0;
    unsafe {
        // BINARY_TBL is 2.
        fitsio_sys::ffcrtb(
            uvfits.as_raw(),            /* I - FITS file pointer                        */
            2,                          /* I - type of table to create                  */
            0,                          /* I - number of rows in the table              */
            11,                         /* I - number of columns in the table           */
            c_col_names.as_mut_ptr(),   /* I - name of each column                      */
            c_col_formats.as_mut_ptr(), /* I - value of TFORMn keyword for each column  */
            c_col_units.as_mut_ptr(),   /* I - value of TUNITn keyword for each column  */
            extname.as_ptr(),           /* I - value of EXTNAME keyword, if any         */
            &mut status,                /* IO - error status                            */
        );
    }
    fits_check_status(status)?;

    // Open the newly-created HDU.
    let hdu = uvfits.hdu(1)?;

    // Set ARRAYX, Y and Z to the MWA's coordinates in XYZ. The results here are
    // slightly different to those given by cotter. This is at least partly due
    // to different constants (the altitude is definitely slightly different),
    // but possibly also because ERFA is more accurate than cotter's
    // "homebrewed" Geodetic2XYZ.
    let mut mwa_xyz: [f64; 3] = [0.0; 3];
    unsafe {
        status = eraGd2gc(
            ERFA_WGS84 as i32,             // ellipsoid identifier (Note 1)
            mwalib::MWA_LONGITUDE_RADIANS, // longitude (radians, east +ve)
            mwalib::MWA_LATITUDE_RADIANS,  // latitude (geodetic, radians, Note 3)
            mwalib::MWA_ALTITUDE_METRES,   // height above ellipsoid (geodetic, Notes 2,3)
            mwa_xyz.as_mut_ptr(),          // geocentric vector (Note 2)
        );
    }
    if status != 0 {
        return Err(UvfitsError::ERFA {
            source_file: file!().to_string(),
            source_line: line!(),
            status,
            function: "eraGd2gc".to_string(),
        });
    }

    hdu.write_key(uvfits, "ARRAYX", mwa_xyz[0])?;
    hdu.write_key(uvfits, "ARRAYY", mwa_xyz[1])?;
    hdu.write_key(uvfits, "ARRAYZ", mwa_xyz[2])?;

    hdu.write_key(uvfits, "FREQ", centre_freq)?;

    // Get the Greenwich mean sidereal time from ERFA. This strange way of
    // calling the function is what PAL does to get the GMST.
    let mjd = start_epoch.as_mjd_utc_days();
    let gmst = unsafe { eraGmst06(ERFA_DJM0, mjd.floor(), ERFA_DJM0, mjd.floor()) }.to_degrees();
    hdu.write_key(uvfits, "GSTIA0", gmst)?;
    hdu.write_key(uvfits, "DEGPDY", 3.60985e2)?; // Earth's rotation rate

    let date_truncated = get_truncated_date_string(start_epoch);
    hdu.write_key(uvfits, "RDATE", date_truncated)?;

    hdu.write_key(uvfits, "POLARX", 0.0)?;
    hdu.write_key(uvfits, "POLARY", 0.0)?;
    hdu.write_key(uvfits, "UT1UTC", 0.0)?;
    hdu.write_key(uvfits, "DATUTC", 0.0)?;

    hdu.write_key(uvfits, "TIMSYS", "UTC")?;
    hdu.write_key(uvfits, "ARRNAM", "MWA")?;
    hdu.write_key(uvfits, "NUMORB", 0)?; // number of orbital parameters in table
    hdu.write_key(uvfits, "NOPCAL", 3)?; // Nr pol calibration values / IF(N_pcal)
    hdu.write_key(uvfits, "FREQID", -1)?; // Frequency setup number
    hdu.write_key(uvfits, "IATUTC", 33.0)?;

    // Assume the station coordinates are "right handed".
    hdu.write_key(uvfits, "XYZHAND", "RIGHT")?;

    let c_antenna_names = rust_strings_to_c_strings(antenna_names)?;

    // Write to the table row by row.
    for (i, pos) in positions.outer_iter().enumerate() {
        let row = i as i64 + 1;
        unsafe {
            // ANNAME. ffpcls = fits_write_col_str
            fitsio_sys::ffpcls(
                uvfits.as_raw(),                   /* I - FITS file pointer                       */
                1,                                 /* I - number of column to write (1 = 1st col) */
                row,                               /* I - first row to write (1 = 1st row)        */
                1,                                 /* I - first vector element to write (1 = 1st) */
                1,                                 /* I - number of strings to write              */
                [c_antenna_names[i]].as_mut_ptr(), /* I - array of pointers to strings            */
                &mut status,                       /* IO - error status                           */
            );
            fits_check_status(status)?;

            // STABXYZ. ffpcld = fits_write_col_dbl
            let xyz = XYZ {
                x: pos[0] - mwa_xyz[0],
                y: pos[1] - mwa_xyz[1],
                z: pos[2] - mwa_xyz[2],
            }
            .rotate_mwa(-1);
            let mut c_xyz = [xyz.x, xyz.y, xyz.z];
            fitsio_sys::ffpcld(
                uvfits.as_raw(),    /* I - FITS file pointer                       */
                2,                  /* I - number of column to write (1 = 1st col) */
                row,                /* I - first row to write (1 = 1st row)        */
                1,                  /* I - first vector element to write (1 = 1st) */
                3,                  /* I - number of values to write               */
                c_xyz.as_mut_ptr(), /* I - array of values to write                */
                &mut status,        /* IO - error status                           */
            );
            fits_check_status(status)?;

            // NOSTA. ffpclk = fits_write_col_int
            fitsio_sys::ffpclk(
                uvfits.as_raw(),           /* I - FITS file pointer                       */
                3,                         /* I - number of column to write (1 = 1st col) */
                row,                       /* I - first row to write (1 = 1st row)        */
                1,                         /* I - first vector element to write (1 = 1st) */
                1,                         /* I - number of values to write               */
                [row as i32].as_mut_ptr(), /* I - array of values to write                */
                &mut status,               /* IO - error status                           */
            );
            fits_check_status(status)?;

            // MNTSTA
            fitsio_sys::ffpclk(
                uvfits.as_raw(),  /* I - FITS file pointer                       */
                4,                /* I - number of column to write (1 = 1st col) */
                row,              /* I - first row to write (1 = 1st row)        */
                1,                /* I - first vector element to write (1 = 1st) */
                1,                /* I - number of values to write               */
                [0].as_mut_ptr(), /* I - array of values to write                */
                &mut status,      /* IO - error status                           */
            );
            fits_check_status(status)?;

            // No row 5?
            // POLTYA
            fitsio_sys::ffpcls(
                uvfits.as_raw(), /* I - FITS file pointer                       */
                6,               /* I - number of column to write (1 = 1st col) */
                row,             /* I - first row to write (1 = 1st row)        */
                1,               /* I - first vector element to write (1 = 1st) */
                1,               /* I - number of strings to write              */
                [CString::new("X").unwrap().into_raw()].as_mut_ptr(), /* I - array of pointers to strings            */
                &mut status, /* IO - error status                           */
            );
            fits_check_status(status)?;

            // POLAA. ffpcle = fits_write_col_flt
            fitsio_sys::ffpcle(
                uvfits.as_raw(),    /* I - FITS file pointer                       */
                7,                  /* I - number of column to write (1 = 1st col) */
                row,                /* I - first row to write (1 = 1st row)        */
                1,                  /* I - first vector element to write (1 = 1st) */
                1,                  /* I - number of values to write               */
                [0.0].as_mut_ptr(), /* I - array of values to write                */
                &mut status,        /* IO - error status                           */
            );
            fits_check_status(status)?;

            // POL calA
            fitsio_sys::ffpcle(
                uvfits.as_raw(),    /* I - FITS file pointer                       */
                8,                  /* I - number of column to write (1 = 1st col) */
                row,                /* I - first row to write (1 = 1st row)        */
                1,                  /* I - first vector element to write (1 = 1st) */
                1,                  /* I - number of values to write               */
                [0.0].as_mut_ptr(), /* I - array of values to write                */
                &mut status,        /* IO - error status                           */
            );
            fits_check_status(status)?;

            // POLTYB
            fitsio_sys::ffpcls(
                uvfits.as_raw(), /* I - FITS file pointer                       */
                9,               /* I - number of column to write (1 = 1st col) */
                row,             /* I - first row to write (1 = 1st row)        */
                1,               /* I - first vector element to write (1 = 1st) */
                1,               /* I - number of strings to write              */
                [CString::new("Y").unwrap().into_raw()].as_mut_ptr(), /* I - array of pointers to strings            */
                &mut status, /* IO - error status                           */
            );
            fits_check_status(status)?;

            // POLAB.
            fitsio_sys::ffpcle(
                uvfits.as_raw(),     /* I - FITS file pointer                       */
                10,                  /* I - number of column to write (1 = 1st col) */
                row,                 /* I - first row to write (1 = 1st row)        */
                1,                   /* I - first vector element to write (1 = 1st) */
                1,                   /* I - number of values to write               */
                [90.0].as_mut_ptr(), /* I - array of values to write                */
                &mut status,         /* IO - error status                           */
            );
            fits_check_status(status)?;

            // POL calB
            fitsio_sys::ffpcle(
                uvfits.as_raw(),    /* I - FITS file pointer                       */
                11,                 /* I - number of column to write (1 = 1st col) */
                row,                /* I - first row to write (1 = 1st row)        */
                1,                  /* I - first vector element to write (1 = 1st) */
                1,                  /* I - number of values to write               */
                [0.0].as_mut_ptr(), /* I - array of values to write                */
                &mut status,        /* IO - error status                           */
            );
            fits_check_status(status)?;
        }
    }

    Ok(())
}

/// Write a prepared vector of floats into a uvfits random group.
///
/// `uvfits` must have been opened in write mode and currently have HDU 0 open.
pub fn write_uvfits_vis(
    uvfits: &mut FitsFile,
    row_num: i64,
    mut row: Vec<f32>,
) -> Result<(), UvfitsError> {
    let mut status = 0;
    unsafe {
        fitsio_sys::ffpgpe(
            uvfits.as_raw(),  /* I - FITS file pointer                      */
            1 + row_num,      /* I - group to write(1 = 1st group)          */
            1,                /* I - first vector element to write(1 = 1st) */
            row.len() as i64, /* I - number of values to write              */
            row.as_mut_ptr(), /* I - array of values that are written       */
            &mut status,      /* IO - error status                          */
        );
    }
    fits_check_status(status)?;
    Ok(())
}

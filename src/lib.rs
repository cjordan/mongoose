// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

pub mod coords;
pub mod cotter;
pub mod erfa;
pub mod fits;
pub mod ms;
pub mod rts;
pub mod time;

use lazy_static::lazy_static;

lazy_static! {
/// The speed of light [m/s]
pub static ref VELC: f64 = 299792458.0;
/// The speed of light [m/s]
pub static ref SVELC: f32 = 299792458.0;

/// PI.
pub static ref DPI: f64 = std::f64::consts::PI;
/// 2 * PI.
pub static ref D2PI: f64 = 2.0 * std::f64::consts::PI;

/// PI.
pub static ref SPI: f32 = std::f32::consts::PI;
/// 2 * PI.
pub static ref S2PI: f32 = 2.0 * std::f32::consts::PI;
}

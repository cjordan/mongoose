// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use criterion::*;

use mongoose::cotter::*;

fn cotter_occupancy(c: &mut Criterion) {
    // The mwaf file is zipped to save space in git. Unzip it to a temporary spot.
    let mut mwaf = tempfile::NamedTempFile::new().unwrap();
    let mut z =
        zip::ZipArchive::new(std::fs::File::open("tests/1065880128_01.mwaf.zip").unwrap()).unwrap();
    let mut z_mwaf = z.by_index(0).unwrap();
    std::io::copy(&mut z_mwaf, &mut mwaf).unwrap();

    c.bench_function("calculating cotter occupancy", |b| {
        b.iter(|| {
            Occupancy::new(&mwaf).unwrap();
        })
    });
}

criterion_group!(benches, cotter_occupancy);
criterion_main!(benches);

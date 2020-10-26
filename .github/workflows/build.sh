#!/bin/bash

set -eux

# I don't know why, but I need to reinstall Rust. Probably something to do with
# GitHub overriding env variables.
curl https://sh.rustup.rs -sSf | sh -s -- -y

# Install dependencies
cd /root
curl -L "https://heasarc.gsfc.nasa.gov/FTP/software/fitsio/c/cfitsio-3.49.tar.gz" -o cfitsio-3.49.tar.gz
tar -xf cfitsio-3.49.tar.gz
cd cfitsio-3.49
# The user's reference guide states that using SSSE3 and SSE2 can make reading
# or writing FITS images 20-30% faster(!). Enabling SSSE3 and SSE2 could cause
# portability problems, but it's unlikely that anyone is using such a CPU...
# https://stackoverflow.com/questions/52858556/most-recent-processor-without-support-of-ssse3-instructions
CFLAGS="-O3" ./configure --prefix="${PWD}" --enable-reentrant --enable-ssse3 --enable-sse2 --disable-curl
make -j install
PKG_CONFIG_PATH=/root/cfitsio-3.49

cd /root
curl -L "https://github.com/liberfa/erfa/releases/download/v1.7.2/erfa-1.7.2.tar.gz" -o erfa-1.7.2.tar.gz
tar -xf erfa-1.7.2.tar.gz
cd erfa-1.7.2
CFLAGS="-O3" ./configure --prefix="${PWD}"
make -j install
PKG_CONFIG_PATH+=:/root/erfa-1.72

# Build
cd /io
PKG_CONFIG_ALL_STATIC=1 cargo build --release

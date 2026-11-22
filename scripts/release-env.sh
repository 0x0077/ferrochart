# Sourced by the package build scripts. Shipped binaries must not contain the
# build machine's paths: panic locations of dependencies name their sources
# under CARGO_HOME, so those prefixes are remapped. rustc applies the last
# matching --remap-path-prefix, so the most specific prefix comes last.
# Release packages build in their own target directory, so the changed
# RUSTFLAGS do not invalidate the development builds in target/.
cargo_home=${CARGO_HOME:-$HOME/.cargo}
export RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$HOME=/home --remap-path-prefix=$cargo_home=/cargo --remap-path-prefix=$(pwd -P)=/ferrochart"
export CARGO_TARGET_DIR="$(pwd -P)/target/package"
# Whole-program optimization for shipped binaries: the wasm package is about
# 100 KB (gzip) smaller with it (P7). Development builds keep the defaults.
export CARGO_PROFILE_RELEASE_LTO=fat
export CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1

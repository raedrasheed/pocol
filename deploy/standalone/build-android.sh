#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
: "${ANDROID_HOME:?Android SDK required}"
ndk="$ANDROID_HOME/ndk/27.2.12479018/toolchains/llvm/prebuilt/linux-x86_64/bin"
test -x "$ndk/aarch64-linux-android26-clang"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$ndk/aarch64-linux-android26-clang"
export CC_aarch64_linux_android="$ndk/aarch64-linux-android26-clang"
export AR_aarch64_linux_android="$ndk/llvm-ar"
export RUSTFLAGS='-C link-arg=-Wl,-z,max-page-size=16384'
cargo +1.99.1 build --locked --release --lib --target aarch64-linux-android --manifest-path "$root/prototypes/pocold/Cargo.toml"
out="$root/prototypes/pocol-android/app/src/main/jniLibs/arm64-v8a"
mkdir -p "$out"
cp "$root/prototypes/pocold/target/aarch64-linux-android/release/libpocold.so" "$out/"
"$ndk/aarch64-linux-android26-clang" -shared -fPIC -Wall -Wextra -Werror -Wl,-z,max-page-size=16384 -Wl,-soname,libpocol_bridge.so \
  "$root/prototypes/pocol-android/app/src/main/cpp/bridge.c" -L"$out" -lpocold -o "$out/libpocol_bridge.so"
# Release builds stay unsigned until the operator supplies the existing app signer.
# Never generate a fresh debug key and present that APK as an update.
gradle --no-daemon -p "$root/prototypes/pocol-android" assembleRelease

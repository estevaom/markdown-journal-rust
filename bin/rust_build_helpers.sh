#!/usr/bin/env bash
set -euo pipefail

# Ensures macOS Rust/C++ builds can resolve libc++ headers on systems where
# Command Line Tools do not provide complete headers in the default include path.
prepare_macos_cxx_env() {
  if [ "$(uname -s)" != "Darwin" ]; then
    return 0
  fi

  if ! command -v xcrun >/dev/null 2>&1; then
    return 0
  fi

  local sdk_root sdk_cxx_include
  sdk_root="$(xcrun --sdk macosx --show-sdk-path 2>/dev/null || true)"
  if [ -z "$sdk_root" ]; then
    return 0
  fi

  sdk_cxx_include="$sdk_root/usr/include/c++/v1"
  if [ ! -d "$sdk_cxx_include" ]; then
    return 0
  fi

  export SDKROOT="$sdk_root"
  case ":${CPLUS_INCLUDE_PATH:-}:" in
    *":$sdk_cxx_include:"*) ;;
    *) export CPLUS_INCLUDE_PATH="$sdk_cxx_include${CPLUS_INCLUDE_PATH:+:$CPLUS_INCLUDE_PATH}" ;;
  esac

  case " ${CXXFLAGS:-} " in
    *" -isystem $sdk_cxx_include "*) ;;
    *) export CXXFLAGS="-isystem $sdk_cxx_include${CXXFLAGS:+ $CXXFLAGS}" ;;
  esac
}

cargo_build_release() {
  local project_dir="$1"
  shift

  (
    cd "$project_dir"
    prepare_macos_cxx_env
    cargo build --release "$@"
  )
}

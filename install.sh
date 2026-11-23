#!/bin/sh
# Installs the retros binary from GitHub Releases on Linux and macOS.
#
#   curl -fsSL https://raw.githubusercontent.com/wevm/retros/main/install.sh | sh
#
# RETROS_VERSION selects a release, such as 0.1.0, and defaults to the latest one. RETROS_INSTALL
# selects the directory, and defaults to ~/.retros, so the binary goes to ~/.retros/bin/retros.
set -eu

repo=wevm/retros
version=${RETROS_VERSION:-latest}
dir=${RETROS_INSTALL:-$HOME/.retros}

fail() {
  echo "error: $1" >&2
  exit 1
}

# Downloads the URL $1 to the file $2.
download() {
  if command -v curl > /dev/null 2>&1; then
    curl --fail --silent --show-error --location --output "$2" "$1"
  elif command -v wget > /dev/null 2>&1; then
    wget --quiet --output-document "$2" "$1"
  else
    fail "install needs curl or wget"
  fi
}

# Prints the SHA-256 digest of the file $1.
digest() {
  if command -v sha256sum > /dev/null 2>&1; then
    sha256sum "$1" | cut -d ' ' -f 1
  elif command -v shasum > /dev/null 2>&1; then
    shasum -a 256 "$1" | cut -d ' ' -f 1
  else
    fail "install needs sha256sum or shasum to check the download"
  fi
}

# The glibc build links against glibc 2.28. Systems with an older glibc or with musl, such as
# Alpine, get the static musl build.
libc() {
  glibc=$(getconf GNU_LIBC_VERSION 2> /dev/null | cut -d ' ' -f 2) || glibc=
  major=${glibc%%.*}
  minor=${glibc#*.}
  minor=${minor%%.*}

  case "$major.$minor" in
    *[!0-9.]* | .*) echo musl ;;
    *) if [ "$major" -gt 2 ] || { [ "$major" -eq 2 ] && [ "$minor" -ge 28 ]; }; then echo gnu; else echo musl; fi ;;
  esac
}

arch=$(uname -m)
case "$(uname -s)" in
  Darwin)
    # A shell under Rosetta reports x86_64 on Apple silicon, which runs the arm64 build natively.
    if [ "$arch" = x86_64 ] && [ "$(sysctl -n sysctl.proc_translated 2> /dev/null)" = 1 ]; then
      arch=arm64
    fi
    os=apple-darwin
    ;;
  Linux) os=unknown-linux-$(libc) ;;
  *) fail "Retros has no build for $(uname -s). Install it with Cargo instead: cargo install --locked --git https://github.com/$repo retros" ;;
esac

case "$arch" in
  x86_64 | amd64) arch=x86_64 ;;
  arm64 | aarch64) arch=aarch64 ;;
  *) fail "Retros has no build for $arch. Install it with Cargo instead: cargo install --locked --git https://github.com/$repo retros" ;;
esac

asset=retros-$arch-$os.tar.gz
case "$version" in
  latest) url=https://github.com/$repo/releases/latest/download ;;
  *) url=https://github.com/$repo/releases/download/v${version#v} ;;
esac

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

echo "Downloading $url/$asset"
download "$url/$asset" "$tmp/$asset"
download "$url/sha256sums.txt" "$tmp/sha256sums.txt"

expected=$(awk -v asset="$asset" '$2 == asset { print $1 }' "$tmp/sha256sums.txt")
[ -n "$expected" ] || fail "sha256sums.txt has no digest for $asset"
[ "$(digest "$tmp/$asset")" = "$expected" ] || fail "the digest of $asset does not match sha256sums.txt"

tar -xzf "$tmp/$asset" -C "$tmp" retros
mkdir -p "$dir/bin"
mv "$tmp/retros" "$dir/bin/retros"
chmod +x "$dir/bin/retros"

echo "Installed $("$dir/bin/retros" --version) to $dir/bin/retros"
case ":$PATH:" in
  *":$dir/bin:"*) ;;
  *) echo "Add it to PATH in your shell profile: export PATH=\"$dir/bin:\$PATH\"" ;;
esac

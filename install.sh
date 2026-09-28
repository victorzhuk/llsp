#!/bin/sh
# Installs llsp from GitHub releases:
#
#   curl -fsSL https://github.com/victorzhuk/llsp/releases/latest/download/install.sh | sh
#
# LLSP_VERSION      release tag, e.g. v0.1.0 (default: latest)
# LLSP_INSTALL_DIR  target directory (default: ~/.local/bin)
# LLSP_BASE_URL     download from this URL instead of GitHub releases
set -eu

repo="https://github.com/victorzhuk/llsp"
version="${LLSP_VERSION:-latest}"
dir="${LLSP_INSTALL_DIR:-$HOME/.local/bin}"

fail() {
	echo "llsp install: $*" >&2
	exit 1
}

case "$(uname -s)" in
Linux) os=unknown-linux-musl ;;
Darwin) os=apple-darwin ;;
*) fail "unsupported OS $(uname -s); download a build from $repo/releases" ;;
esac

arch=$(uname -m)
# A shell running under Rosetta reports x86_64 on Apple silicon; prefer the native build.
if [ "$os" = apple-darwin ] && [ "$(sysctl -n hw.optional.arm64 2>/dev/null || true)" = 1 ]; then
	arch=arm64
fi
case "$arch" in
x86_64 | amd64) arch=x86_64 ;;
aarch64 | arm64) arch=aarch64 ;;
*) fail "unsupported CPU $arch; download a build from $repo/releases" ;;
esac
target="$arch-$os"

if [ -n "${LLSP_BASE_URL:-}" ]; then
	base="$LLSP_BASE_URL"
elif [ "$version" = latest ]; then
	base="$repo/releases/latest/download"
else
	base="$repo/releases/download/$version"
fi

fetch() {
	if command -v curl >/dev/null 2>&1; then
		curl -fsSL "$1" -o "$2"
	elif command -v wget >/dev/null 2>&1; then
		wget -qO "$2" "$1"
	else
		fail "curl or wget is required"
	fi
}

sha256() {
	if command -v sha256sum >/dev/null 2>&1; then
		sha256sum "$1" | cut -d ' ' -f 1
	elif command -v shasum >/dev/null 2>&1; then
		shasum -a 256 "$1" | cut -d ' ' -f 1
	else
		fail "sha256sum or shasum is required"
	fi
}

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
trap 'exit 130' INT TERM

archive="llsp-$target.tar.gz"
echo "llsp install: downloading $archive ($version)"
fetch "$base/$archive" "$tmp/$archive" || fail "cannot download $base/$archive"
fetch "$base/SHA256SUMS" "$tmp/SHA256SUMS" || fail "cannot download $base/SHA256SUMS"

expected=$(awk -v f="$archive" '$2 == f || $2 == "*" f { print $1 }' "$tmp/SHA256SUMS")
[ -n "$expected" ] || fail "$archive is not listed in SHA256SUMS"
[ "$(sha256 "$tmp/$archive")" = "$expected" ] || fail "checksum mismatch for $archive"

tar -xzf "$tmp/$archive" -C "$tmp"
mkdir -p "$dir"
cp "$tmp/llsp-$target/llsp" "$dir/.llsp.tmp"
chmod 755 "$dir/.llsp.tmp"
mv -f "$dir/.llsp.tmp" "$dir/llsp"

echo "llsp install: installed $("$dir/llsp" --version) to $dir/llsp"
case ":$PATH:" in
*":$dir:"*) ;;
*) echo "llsp install: $dir is not in PATH; add it in your shell profile" ;;
esac

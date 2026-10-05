#!/bin/sh
# Build Spool.app and zip it as a distributable macOS artifact.
#
# Intentionally dependency-free: it uses only tools that ship with macOS
# (cargo, sips, iconutil, ditto). No packaging framework, no installer.
#
# Usage:
#   app/scripts/package-macos.sh
#
# Output:
#   dist/Spool.app
#   dist/Spool-macos-<arch>.zip

set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
app_dir=$(dirname -- "$script_dir")
repo_root=$(dirname -- "$app_dir")
resources_dir="$app_dir/resources"
out_dir="${OUT_DIR:-$repo_root/dist}"

app_name=Spool
bundle_id=in.gdgtiet.spool

die() {
	printf 'package-macos: %s\n' "$1" >&2
	exit 1
}

# macOS-only: the bundle layout, .icns pipeline and ditto are all macOS tools.
[ "$(uname -s)" = "Darwin" ] || die 'this script only runs on macOS'

arch=$(uname -m)

# Single source of truth for the version: the Cargo package version.
version=$(sed -n '/^\[package\]/,/^\[/s/^version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' \
	"$app_dir/Cargo.toml" | head -1)
[ -n "$version" ] || die "could not read [package] version from $app_dir/Cargo.toml"

printf '==> version %s (from app/Cargo.toml)\n' "$version"
printf '==> building %s (release, %s)\n' "$app_name" "$arch"
(cd "$app_dir" && cargo build --release --locked)

built_binary="$app_dir/target/release/$app_name"
[ -x "$built_binary" ] || die "expected executable at $built_binary"

app_bundle="$out_dir/$app_name.app"
contents="$app_bundle/Contents"

rm -rf "$app_bundle"
mkdir -p "$contents/MacOS" "$contents/Resources"

# Executable
cp "$built_binary" "$contents/MacOS/$app_name"
chmod 755 "$contents/MacOS/$app_name"

# Info.plist. CFBundleShortVersionString/CFBundleVersion come from the Cargo
# version; LSMinimumSystemVersion is read back from the built binary so the
# bundle cannot claim support the toolchain did not target.
min_version=$(otool -l "$built_binary" |
	awk '/LC_BUILD_VERSION/ { seen = 1 } seen && $1 == "minos" { print $2; exit }')
[ -n "$min_version" ] || die "could not read minos from $built_binary"

sed -e "s/@VERSION@/$version/g" -e "s/@MIN_VERSION@/$min_version/g" \
	"$resources_dir/Info.plist.in" >"$contents/Info.plist"
plutil -lint "$contents/Info.plist" >/dev/null
plutil -extract CFBundleIdentifier raw "$contents/Info.plist" | grep -qx "$bundle_id" ||
	die "CFBundleIdentifier in $contents/Info.plist is not $bundle_id"

# Icon: 1024px source PNG -> iconset -> Spool.icns
iconset=$(mktemp -d)/Spool.iconset
trap 'rm -rf "$(dirname -- "$iconset")"' EXIT
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
	sips -z "$size" "$size" "$resources_dir/icon-1024.png" \
		--out "$iconset/icon_${size}x${size}.png" >/dev/null
	sips -z "$((size * 2))" "$((size * 2))" "$resources_dir/icon-1024.png" \
		--out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$contents/Resources/$app_name.icns"

# Ad-hoc signature only: it binds Info.plist and seals Resources so the bundle
# is internally consistent after extraction. It is not a Developer ID signature
# and is not notarized; see docs/development/getting-started.md.
codesign --force --sign - "$app_bundle"
codesign --verify --strict "$app_bundle"

# Archive. ditto preserves bundle layout, permissions and symlinks; plain
# `zip` does not, and a bundle with the wrong permissions will not launch.
# --norsrc/--noextattr drop extended attributes and resource forks rather than
# sequestering them into __MACOSX/ entries.
artifact="$out_dir/$app_name-macos-$arch.zip"
rm -f "$artifact"
ditto -c -k --keepParent --norsrc --noextattr "$app_bundle" "$artifact"

printf '==> %s\n' "$app_bundle"
printf '==> %s\n' "$artifact"

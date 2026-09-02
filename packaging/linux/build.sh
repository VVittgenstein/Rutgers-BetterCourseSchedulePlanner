#!/usr/bin/env bash

set -Eeuo pipefail
IFS=$'\n\t'
umask 022

readonly PACKAGE_ID='LINUX_PUBLIC_DEPLOYMENT_PACKAGE'
readonly TARGET='x86_64-unknown-linux-gnu'
readonly RELEASE_VERSION='0.1.2'
readonly ARCHIVE_NAME='rbcsp-linux-x86_64-0.1.2.tar.gz'

usage() {
  printf 'usage: build.sh --source-root PATH [--source-commit SHA] [--output-root PATH] [--build-root PATH]\n' >&2
  exit 2
}

die() {
  printf 'linux-package-build: %s\n' "$*" >&2
  exit 1
}

require_file() {
  [[ -f "$1" && ! -L "$1" ]] || die "required regular file is missing: $1"
}

require_directory() {
  [[ -d "$1" && ! -L "$1" ]] || die "required regular directory is missing: $1"
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || die "required command is unavailable: $1"
}

capture_version() {
  local command_name="$1" expected="$2" output version
  shift 2
  output="$("$command_name" "$@")" || die "could not read the version from $command_name"
  version="$(printf '%s\n' "$output" | awk 'NR == 1 { value=(NF == 1 ? $1 : $2); sub(/^v/, "", value); print value }')"
  [[ "$version" == "$expected" ]] || \
    die "unexpected $command_name version: expected $expected, found ${version:-unknown}"
}

SOURCE_ROOT=''
REQUESTED_SOURCE_COMMIT=''
OUTPUT_ROOT=''
BUILD_ROOT=''
while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --source-root)
      [[ "$#" -ge 2 ]] || usage
      SOURCE_ROOT="$2"
      shift 2
      ;;
    --source-commit)
      [[ "$#" -ge 2 ]] || usage
      REQUESTED_SOURCE_COMMIT="$2"
      shift 2
      ;;
    --output-root)
      [[ "$#" -ge 2 ]] || usage
      OUTPUT_ROOT="$2"
      shift 2
      ;;
    --build-root)
      [[ "$#" -ge 2 ]] || usage
      BUILD_ROOT="$2"
      shift 2
      ;;
    *) usage ;;
  esac
done

[[ -n "$SOURCE_ROOT" ]] || usage

readonly REPOSITORY_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)"
require_command realpath
SOURCE_ROOT="$(realpath -e -- "$SOURCE_ROOT")"
OUTPUT_ROOT="$(realpath -m -- "${OUTPUT_ROOT:-$REPOSITORY_ROOT/release/0.1.2}")"
BUILD_ROOT="$(realpath -m -- "${BUILD_ROOT:-$REPOSITORY_ROOT/.cache/product-build/linux}")"
readonly SOURCE_ROOT OUTPUT_ROOT BUILD_ROOT

readonly ALLOWED_BUILD_PARENT="$(realpath -m -- "$REPOSITORY_ROOT/.cache/product-build")"
case "$BUILD_ROOT/" in
  "$ALLOWED_BUILD_PARENT"/*) ;;
  *) die "--build-root must remain below $ALLOWED_BUILD_PARENT" ;;
esac
[[ "$BUILD_ROOT" != "$ALLOWED_BUILD_PARENT" ]] || die '--build-root must name a child directory'
case "$SOURCE_ROOT/" in
  "$BUILD_ROOT"/*) die '--build-root must not contain --source-root' ;;
esac
case "$BUILD_ROOT/" in
  "$SOURCE_ROOT"/*) die '--build-root must not be inside --source-root' ;;
esac

readonly NODE_BIN="${NODE_BIN:-node}"
readonly NPM_BIN="${NPM_BIN:-npm}"
readonly CARGO_BIN="${CARGO_BIN:-cargo}"
readonly RUSTC_BIN="${RUSTC_BIN:-rustc}"
readonly CARGO_CYCLONEDX_BIN="${CARGO_CYCLONEDX_BIN:-cargo-cyclonedx}"
readonly CARGO_ABOUT_BIN="${CARGO_ABOUT_BIN:-cargo-about}"
readonly GIT_BIN="${GIT_BIN:-git}"
readonly TAR_BIN="${TAR_BIN:-tar}"
readonly GZIP_BIN="${GZIP_BIN:-gzip}"

for command_name in "$NODE_BIN" "$NPM_BIN" "$CARGO_BIN" "$RUSTC_BIN" \
  "$CARGO_CYCLONEDX_BIN" "$CARGO_ABOUT_BIN" "$GIT_BIN" "$TAR_BIN" \
  "$GZIP_BIN" awk cp find grep install mkdir mv rm sha256sum sort; do
  require_command "$command_name"
done
"$TAR_BIN" --version | grep -q 'GNU tar' || die 'GNU tar is required for deterministic archives'

require_directory "$SOURCE_ROOT"
source_commit="$("$GIT_BIN" -C "$SOURCE_ROOT" rev-parse --verify HEAD)"
[[ "$source_commit" =~ ^([0-9a-f]{40}|[0-9a-f]{64})$ ]] || \
  die '--source-root HEAD is not a full hexadecimal commit id'
if [[ -n "$REQUESTED_SOURCE_COMMIT" ]]; then
  [[ "$REQUESTED_SOURCE_COMMIT" =~ ^([0-9a-f]{40}|[0-9a-f]{64})$ ]] || \
    die '--source-commit must be a full lowercase hexadecimal commit id'
  requested_commit="$("$GIT_BIN" -C "$SOURCE_ROOT" rev-parse --verify "${REQUESTED_SOURCE_COMMIT}^{commit}")" || \
    die '--source-commit does not resolve to a commit'
  [[ "$requested_commit" == "$REQUESTED_SOURCE_COMMIT" ]] || \
    die '--source-commit must be the canonical full commit id'
  [[ "$source_commit" == "$REQUESTED_SOURCE_COMMIT" ]] || \
    die "--source-root HEAD is $source_commit, not requested commit $REQUESTED_SOURCE_COMMIT"
elif "$GIT_BIN" -C "$SOURCE_ROOT" symbolic-ref --quiet HEAD >/dev/null 2>&1; then
  die '--source-root must be a detached checkout unless --source-commit is supplied'
fi
[[ -z "$("$GIT_BIN" -C "$SOURCE_ROOT" status --porcelain --untracked-files=all)" ]] || \
  die '--source-root must be a clean checkout'

readonly RELEASE_INPUTS="$SOURCE_ROOT/packaging/release-inputs.json"
readonly METADATA_GENERATOR="$SOURCE_ROOT/packaging/generate-release-metadata.mjs"
readonly ABOUT_CONFIG="$SOURCE_ROOT/packaging/about.toml"
readonly MIT_TEMPLATE="$SOURCE_ROOT/packaging/licenses/MIT.txt"
for required in "$RELEASE_INPUTS" "$METADATA_GENERATOR" "$ABOUT_CONFIG" \
  "$MIT_TEMPLATE" "$SOURCE_ROOT/LICENSE" "$SOURCE_ROOT/Cargo.lock" \
  "$SOURCE_ROOT/frontend/package-lock.json"; do
  require_file "$required"
done
require_directory "$SOURCE_ROOT/deploy/public"

allowlist_text="$("$NODE_BIN" - "$RELEASE_INPUTS" "$PACKAGE_ID" "$TARGET" "$ARCHIVE_NAME" <<'NODE'
const fs = require('node:fs');
const [file, packageId, target, archiveName] = process.argv.slice(2);
const input = JSON.parse(fs.readFileSync(file, 'utf8'));
const matches = input.packages?.filter((entry) => entry.id === packageId) ?? [];
if (input.schemaVersion !== 1 || input.packageCount !== 2 || matches.length !== 1) process.exit(10);
const packageConfig = matches[0];
if (input.releaseVersion !== '0.1.2' || packageConfig.target !== target || packageConfig.archiveName !== archiveName) process.exit(11);
if (!Array.isArray(packageConfig.allowlist) || packageConfig.allowlist.length !== 22) process.exit(12);
const files = [...packageConfig.allowlist].sort();
if (new Set(files).size !== 22 || files.some((fileName) => fileName.startsWith('/') || fileName.includes('..') || fileName.includes('\\'))) process.exit(13);
process.stdout.write(`${files.join('\n')}\n`);
NODE
)" || die 'the frozen Linux package definition is invalid'
mapfile -t EXPECTED_FILES <<< "$allowlist_text"
[[ "${#EXPECTED_FILES[@]}" -eq 22 ]] || die 'the Linux package allowlist must contain exactly 22 files'

capture_version "$RUSTC_BIN" '1.97.0' --version
capture_version "$CARGO_BIN" '1.97.0' --version
capture_version "$NODE_BIN" '24.18.0' --version
npm_version="$("$NPM_BIN" --version)"
[[ "$npm_version" == '11.16.0' ]] || die "unexpected npm version: $npm_version"
[[ "$("$CARGO_CYCLONEDX_BIN" cyclonedx --version)" == 'cargo-cyclonedx-cyclonedx 0.5.9' ]] || \
  die 'cargo-cyclonedx 0.5.9 is required'
[[ "$("$CARGO_ABOUT_BIN" --version)" == 'cargo-about 0.9.1' ]] || \
  die 'cargo-about 0.9.1 is required'

source_date_epoch="$("$GIT_BIN" -C "$SOURCE_ROOT" show -s --format=%ct "$source_commit")"
[[ "$source_date_epoch" =~ ^[0-9]+$ ]] || die 'the source commit timestamp is invalid'
export SOURCE_DATE_EPOCH="$source_date_epoch"
export TZ=UTC
export LC_ALL=C
export LANG=C

rm -rf -- "$BUILD_ROOT"
mkdir -p -- "$BUILD_ROOT"
readonly PACKAGE_ROOT="$BUILD_ROOT/package"
readonly TARGET_ROOT="$BUILD_ROOT/cargo-target"
readonly METADATA_ROOT="$BUILD_ROOT/metadata"
readonly METADATA_SOURCE_ROOT="$BUILD_ROOT/metadata-source"
mkdir -p -- "$PACKAGE_ROOT" "$TARGET_ROOT" "$METADATA_ROOT" \
  "$METADATA_SOURCE_ROOT/frontend" "$METADATA_SOURCE_ROOT/packaging/licenses"

(
  cd -- "$SOURCE_ROOT/frontend"
  "$NPM_BIN" ci --no-audit --no-fund
  "$NPM_BIN" run build:public
)

export BCSP_PUBLIC_WEB_DIST="$SOURCE_ROOT/frontend/dist/public"
export CARGO_TARGET_DIR="$TARGET_ROOT"
export CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}"
cargo_home_root="$(realpath -e -- "${CARGO_HOME:-$HOME/.cargo}")"
readonly cargo_home_root
path_remap_cflags="-ffile-prefix-map=$SOURCE_ROOT=. -ffile-prefix-map=$cargo_home_root=.cargo"
export CFLAGS="${CFLAGS:+$CFLAGS }$path_remap_cflags"
export CXXFLAGS="${CXXFLAGS:+$CXXFLAGS }$path_remap_cflags"
unit_separator=$'\x1f'
export CARGO_ENCODED_RUSTFLAGS="-C${unit_separator}link-arg=-Wl,--build-id=none${unit_separator}--remap-path-prefix=$SOURCE_ROOT=.${unit_separator}--remap-path-prefix=$cargo_home_root=.cargo"
"$CARGO_BIN" build \
  --manifest-path "$SOURCE_ROOT/Cargo.toml" \
  --package bcsp-server \
  --target "$TARGET" \
  --release \
  --locked

readonly BUILT_BINARY="$TARGET_ROOT/$TARGET/release/bcsp-server"
require_file "$BUILT_BINARY"
install -D -m 0755 -- "$BUILT_BINARY" "$PACKAGE_ROOT/bin/bcsp-server"
install -m 0644 -- "$BCSP_PUBLIC_WEB_DIST/capability-manifest.json" \
  "$PACKAGE_ROOT/FRONTEND-CAPABILITIES.json"
install -m 0644 -- "$SOURCE_ROOT/LICENSE" "$PACKAGE_ROOT/LICENSE"
printf '%s\n' "$RELEASE_VERSION" > "$PACKAGE_ROOT/VERSION"

for relative in caddy/Caddyfile.example config/bcsp.env.example \
  config/bcsp.env.schema.json docs/operator-runbook.md systemd/bcsp.service; do
  install -D -m 0644 -- "$SOURCE_ROOT/deploy/public/$relative" "$PACKAGE_ROOT/$relative"
done
for relative in ops/backup.sh ops/install.sh ops/lib.sh ops/preflight.sh \
  ops/restore.sh ops/rollback.sh ops/upgrade.sh ops/verify.sh; do
  install -D -m 0755 -- "$SOURCE_ROOT/deploy/public/$relative" "$PACKAGE_ROOT/$relative"
done

readonly CYCLONE_OUTPUT_BASE='.bcsp-rust-sbom'
cleanup_cyclonedx_outputs() {
  local generated
  while IFS= read -r -d '' generated; do
    rm -f -- "$generated"
  done < <(find "$SOURCE_ROOT/apps" "$SOURCE_ROOT/crates" -type f \
    -name "$CYCLONE_OUTPUT_BASE.json" -print0)
}
trap cleanup_cyclonedx_outputs EXIT
(
  cd -- "$SOURCE_ROOT"
  "$CARGO_CYCLONEDX_BIN" cyclonedx \
    --manifest-path "$SOURCE_ROOT/apps/bcsp-server/Cargo.toml" \
    --format json \
    --target "$TARGET" \
    --spec-version 1.5 \
    --license-strict \
    --no-build-deps \
    --override-filename "$CYCLONE_OUTPUT_BASE"
)
readonly RAW_RUST_SBOM="$SOURCE_ROOT/apps/bcsp-server/$CYCLONE_OUTPUT_BASE.json"
require_file "$RAW_RUST_SBOM"
cp -- "$RAW_RUST_SBOM" "$METADATA_ROOT/rust-sbom.json"
cleanup_cyclonedx_outputs
if find "$SOURCE_ROOT/apps" "$SOURCE_ROOT/crates" -type f \
  -name "$CYCLONE_OUTPUT_BASE.json" -print -quit | grep -q .; then
  die 'cargo-cyclonedx left an unexpected generated SBOM in the frozen source checkout'
fi

readonly RAW_RUST_ABOUT="$METADATA_ROOT/rust-about.json"
"$CARGO_ABOUT_BIN" generate \
  --manifest-path "$SOURCE_ROOT/apps/bcsp-server/Cargo.toml" \
  --config "$ABOUT_CONFIG" \
  --target "$TARGET" \
  --frozen \
  --fail \
  --format json \
  --output-file "$RAW_RUST_ABOUT"
require_file "$RAW_RUST_ABOUT"

# The product build remains the clean frozen checkout. This metadata-only view
# adds the post-freeze generator inputs without mutating that checkout.
install -m 0644 -- "$SOURCE_ROOT/Cargo.toml" "$SOURCE_ROOT/Cargo.lock" "$METADATA_SOURCE_ROOT/"
install -m 0644 -- "$SOURCE_ROOT/frontend/package.json" \
  "$SOURCE_ROOT/frontend/package-lock.json" "$METADATA_SOURCE_ROOT/frontend/"
cp -a -- "$SOURCE_ROOT/frontend/node_modules" "$METADATA_SOURCE_ROOT/frontend/node_modules"
install -m 0644 -- "$RELEASE_INPUTS" "$METADATA_SOURCE_ROOT/packaging/release-inputs.json"
install -m 0644 -- "$ABOUT_CONFIG" "$METADATA_SOURCE_ROOT/packaging/about.toml"
install -m 0644 -- "$METADATA_GENERATOR" "$METADATA_SOURCE_ROOT/packaging/generate-release-metadata.mjs"
install -m 0644 -- "$MIT_TEMPLATE" "$METADATA_SOURCE_ROOT/packaging/licenses/MIT.txt"

"$NODE_BIN" "$METADATA_GENERATOR" \
  --source-root "$METADATA_SOURCE_ROOT" \
  --stage-root "$PACKAGE_ROOT" \
  --package-id "$PACKAGE_ID" \
  --target "$TARGET" \
  --binary "$PACKAGE_ROOT/bin/bcsp-server" \
  --source-commit "$source_commit" \
  --source-date-epoch "$source_date_epoch" \
  --rustc-version '1.97.0' \
  --cargo-version '1.97.0' \
  --node-version '24.18.0' \
  --npm-version '11.16.0' \
  --cargo-cyclonedx-version '0.5.9' \
  --cargo-about-version '0.9.1' \
  --raw-cargo-sbom "$METADATA_ROOT/rust-sbom.json" \
  --raw-cargo-about "$RAW_RUST_ABOUT"

[[ -z "$("$GIT_BIN" -C "$SOURCE_ROOT" status --porcelain --untracked-files=all)" ]] || \
  die 'the build left files in the frozen source checkout'

actual_files="$(find "$PACKAGE_ROOT" -type f -printf '%P\n' | sort)"
expected_files="$(printf '%s\n' "${EXPECTED_FILES[@]}")"
[[ "$actual_files" == "$expected_files" ]] || die 'the staged package does not match the exact 22-file allowlist'
[[ -z "$(find "$PACKAGE_ROOT" \( -type l -o \( ! -type d ! -type f \) \) -print -quit)" ]] || \
  die 'the staged package contains a symbolic link or special file'

mkdir -p -- "$OUTPUT_ROOT"
readonly ARCHIVE_PATH="$OUTPUT_ROOT/$ARCHIVE_NAME"
readonly TEMP_ARCHIVE="$OUTPUT_ROOT/.$ARCHIVE_NAME.tmp.$$"
rm -f -- "$TEMP_ARCHIVE"
(
  cd -- "$PACKAGE_ROOT"
  "$TAR_BIN" --format=ustar --mtime="@$source_date_epoch" --owner=0 --group=0 \
    --numeric-owner --mode='u+rwX,go+rX,go-w' -cf - "${EXPECTED_FILES[@]}" |
    "$GZIP_BIN" -n -9 > "$TEMP_ARCHIVE"
)
mv -f -- "$TEMP_ARCHIVE" "$ARCHIVE_PATH"

bash "$SOURCE_ROOT/packaging/linux/verify.sh" \
  --archive "$ARCHIVE_PATH" \
  --source-commit "$source_commit" \
  --source-date-epoch "$source_date_epoch"
printf 'Created %s\nSHA256 %s\n' "$ARCHIVE_PATH" "$(sha256sum "$ARCHIVE_PATH" | awk '{print $1}')"

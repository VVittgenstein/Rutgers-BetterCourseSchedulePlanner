#!/usr/bin/env bash

set -Eeuo pipefail
IFS=$'\n\t'
umask 022
export LC_ALL=C
export LANG=C
export TZ=UTC

readonly EXPECTED_SOURCE_COMMIT='7d5debef005277e4d8f2ed2b9fb2f72c495e62f1'
readonly EXPECTED_SOURCE_DATE_EPOCH='1784109539'
readonly PACKAGE_ID='LINUX_PUBLIC_DEPLOYMENT_PACKAGE'
readonly TARGET='x86_64-unknown-linux-gnu'
readonly RELEASE_VERSION='0.1.0'
readonly ARCHIVE_NAME='rbcsp-linux-x86_64-0.1.0.tar.gz'

usage() {
  printf 'usage: verify.sh --archive PATH\n' >&2
  exit 2
}

die() {
  printf 'linux-package-verify: %s\n' "$*" >&2
  exit 1
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || die "required command is unavailable: $1"
}

ARCHIVE_PATH=''
while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --archive)
      [[ "$#" -ge 2 ]] || usage
      ARCHIVE_PATH="$2"
      shift 2
      ;;
    *) usage ;;
  esac
done
[[ -n "$ARCHIVE_PATH" ]] || usage

readonly REPOSITORY_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)"
readonly NODE_BIN="${NODE_BIN:-node}"
readonly TAR_BIN="${TAR_BIN:-tar}"
readonly GZIP_BIN="${GZIP_BIN:-gzip}"
for command_name in "$NODE_BIN" "$TAR_BIN" "$GZIP_BIN" awk bash basename cmp file find grep \
  mkdir mktemp readelf realpath rm sha256sum sort stat; do
  require_command "$command_name"
done
"$TAR_BIN" --version | grep -q 'GNU tar' || die 'GNU tar is required'

ARCHIVE_PATH="$(realpath -e -- "$ARCHIVE_PATH")"
readonly ARCHIVE_PATH
[[ -f "$ARCHIVE_PATH" && ! -L "$ARCHIVE_PATH" ]] || die 'archive must be a regular file'
[[ "$(basename -- "$ARCHIVE_PATH")" == "$ARCHIVE_NAME" ]] || die "archive name must be $ARCHIVE_NAME"
"$GZIP_BIN" -t -- "$ARCHIVE_PATH"

readonly RELEASE_INPUTS="$REPOSITORY_ROOT/packaging/release-inputs.json"
[[ -f "$RELEASE_INPUTS" ]] || die 'packaging/release-inputs.json is missing'
allowlist_text="$("$NODE_BIN" - "$RELEASE_INPUTS" "$PACKAGE_ID" "$TARGET" "$ARCHIVE_NAME" <<'NODE'
const fs = require('node:fs');
const [file, packageId, target, archiveName] = process.argv.slice(2);
const input = JSON.parse(fs.readFileSync(file, 'utf8'));
const matches = input.packages?.filter((entry) => entry.id === packageId) ?? [];
if (input.schemaVersion !== 1 || input.packageCount !== 2 || matches.length !== 1) process.exit(10);
const packageConfig = matches[0];
if (input.releaseVersion !== '0.1.0' || packageConfig.target !== target || packageConfig.archiveName !== archiveName) process.exit(11);
if (!Array.isArray(packageConfig.allowlist) || packageConfig.allowlist.length !== 20) process.exit(12);
const files = [...packageConfig.allowlist].sort();
if (new Set(files).size !== 20) process.exit(13);
process.stdout.write(`${files.join('\n')}\n`);
NODE
)" || die 'the frozen Linux package definition is invalid'
mapfile -t EXPECTED_FILES <<< "$allowlist_text"

archive_listing="$("$TAR_BIN" -tzf "$ARCHIVE_PATH")"
mapfile -t ARCHIVE_FILES <<< "$archive_listing"
[[ "${#ARCHIVE_FILES[@]}" -eq 20 ]] || die "archive must contain exactly 20 files; found ${#ARCHIVE_FILES[@]}"
[[ "$archive_listing" == "$(printf '%s\n' "${EXPECTED_FILES[@]}")" ]] || \
  die 'archive entries do not match the exact Linux allowlist and order'
for entry in "${ARCHIVE_FILES[@]}"; do
  [[ "$entry" != /* && "$entry" != *'..'* && "$entry" != *'\\'* ]] || \
    die "archive contains an unsafe path: $entry"
done
"$TAR_BIN" -tvzf "$ARCHIVE_PATH" | awk 'substr($1, 1, 1) != "-" { exit 1 }' || \
  die 'archive contains a non-regular entry'

readonly TEMP_ROOT="$(mktemp -d)"
cleanup() {
  rm -rf -- "$TEMP_ROOT"
}
trap cleanup EXIT
readonly CANDIDATE_ROOT="$TEMP_ROOT/candidate"
mkdir -p -- "$CANDIDATE_ROOT"
"$TAR_BIN" -xzf "$ARCHIVE_PATH" -C "$CANDIDATE_ROOT" --no-same-owner --no-same-permissions

actual_files="$(find "$CANDIDATE_ROOT" -type f -printf '%P\n' | sort)"
expected_files="$(printf '%s\n' "${EXPECTED_FILES[@]}")"
[[ "$actual_files" == "$expected_files" ]] || die 'extracted files do not match the exact 20-file allowlist'
[[ -z "$(find "$CANDIDATE_ROOT" -type l -print -quit)" ]] || die 'candidate contains a symbolic link'
[[ -z "$(find "$CANDIDATE_ROOT" ! -type d ! -type f -print -quit)" ]] || die 'candidate contains a special file'
for forbidden in data share tests; do
  [[ ! -e "$CANDIDATE_ROOT/$forbidden" ]] || die "candidate contains forbidden $forbidden/"
done
if find "$CANDIDATE_ROOT" -type f \( -iname '*.map' -o -iname '*.db' -o \
  -iname '*.sqlite' -o -iname '*.sqlite3' -o -iname '*-wal' -o \
  -iname '*-shm' -o -iname '*-journal' \) -print -quit | grep -q .; then
  die 'candidate contains a source map or runtime database artifact'
fi

[[ "$(stat -c '%a' "$CANDIDATE_ROOT/bin/bcsp-server")" == '755' ]] || \
  die 'bin/bcsp-server must have mode 0755'
for script in "$CANDIDATE_ROOT"/ops/*.sh; do
  [[ "$(stat -c '%a' "$script")" == '755' ]] || die "operation script is not mode 0755: $script"
  bash -n -- "$script"
done
bash -c 'source "$1"; bcsp_validate_candidate_root "$2"' _ \
  "$CANDIDATE_ROOT/ops/lib.sh" "$CANDIDATE_ROOT"
for file_name in "${EXPECTED_FILES[@]}"; do
  case "$file_name" in
    bin/bcsp-server|ops/*.sh) ;;
    *) [[ "$(stat -c '%a' "$CANDIDATE_ROOT/$file_name")" == '644' ]] || \
      die "non-executable package file is not mode 0644: $file_name" ;;
  esac
done

file "$CANDIDATE_ROOT/bin/bcsp-server" | grep -Eq 'ELF 64-bit.*x86-64' || \
  die 'bin/bcsp-server is not a Linux x86-64 ELF executable'
readelf -h "$CANDIDATE_ROOT/bin/bcsp-server" | grep -Eq 'Class:[[:space:]]+ELF64' || \
  die 'bin/bcsp-server has the wrong ELF class'
readelf -h "$CANDIDATE_ROOT/bin/bcsp-server" | grep -Eq 'Machine:[[:space:]]+Advanced Micro Devices X86-64' || \
  die 'bin/bcsp-server has the wrong ELF machine'

"$NODE_BIN" - "$CANDIDATE_ROOT" "$PACKAGE_ID" "$TARGET" \
  "$EXPECTED_SOURCE_COMMIT" "$EXPECTED_SOURCE_DATE_EPOCH" "$ARCHIVE_NAME" \
  "$RELEASE_VERSION" <<'NODE'
const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const [root, packageId, target, sourceCommit, sourceDateEpoch, archiveName, releaseVersion] = process.argv.slice(2);
const read = (name) => JSON.parse(fs.readFileSync(path.join(root, name), 'utf8'));
const hash = (name) => crypto.createHash('sha256').update(fs.readFileSync(path.join(root, name))).digest('hex');
const manifest = read('MANIFEST.json');
const provenance = read('BUILD-PROVENANCE.json');
const sbom = read('SBOM.cdx.json');
const allFiles = fs.readdirSync(root, { recursive: true, withFileTypes: true })
  .filter((entry) => entry.isFile())
  .map((entry) => path.relative(root, path.join(entry.parentPath, entry.name)).split(path.sep).join('/'))
  .sort();
const expected = allFiles.filter((name) => name !== 'MANIFEST.json' && name !== 'SHA256SUMS');
if (fs.readFileSync(path.join(root, 'VERSION'), 'utf8') !== `${releaseVersion}\n`) process.exit(20);
if (manifest.schemaVersion !== 1 || manifest.packageId !== packageId || manifest.archiveName !== archiveName || manifest.version !== releaseVersion || manifest.target !== target || manifest.sourceCommit !== sourceCommit || String(manifest.sourceDateEpoch) !== sourceDateEpoch) process.exit(21);
if (JSON.stringify(manifest.excludedFiles) !== JSON.stringify(['MANIFEST.json', 'SHA256SUMS'])) process.exit(22);
if (manifest.fileCount !== expected.length || JSON.stringify(manifest.files.map((entry) => entry.path)) !== JSON.stringify(expected)) process.exit(21);
for (const entry of manifest.files) {
  const stat = fs.statSync(path.join(root, entry.path));
  if (stat.size !== entry.sizeBytes || hash(entry.path) !== entry.sha256) process.exit(23);
}
const sums = fs.readFileSync(path.join(root, 'SHA256SUMS'), 'utf8');
if (sums.includes('\r') || !sums.endsWith('\n')) process.exit(24);
const sumEntries = sums.trimEnd().split('\n').map((line) => /^([0-9a-f]{64})  (.+)$/u.exec(line));
if (sumEntries.some((entry) => entry === null)) process.exit(25);
const sumPaths = sumEntries.map((entry) => entry[2]);
const expectedSumPaths = allFiles.filter((name) => name !== 'SHA256SUMS');
if (new Set(sumPaths).size !== sumPaths.length || JSON.stringify(sumPaths) !== JSON.stringify(expectedSumPaths)) process.exit(26);
for (const entry of sumEntries) if (hash(entry[2]) !== entry[1]) process.exit(27);
if (provenance.schemaVersion !== 1 || provenance.packageId !== packageId || provenance.archiveName !== archiveName || provenance.version !== releaseVersion || provenance.source?.commit !== sourceCommit || provenance.build?.target !== target) process.exit(28);
if (provenance.source?.dateEpoch !== Number(sourceDateEpoch) || provenance.artifact?.path !== 'bin/bcsp-server' || provenance.artifact?.sha256 !== hash('bin/bcsp-server')) process.exit(29);
if (sbom.bomFormat !== 'CycloneDX' || sbom.specVersion !== '1.6' || sbom.version !== 1 || sbom.metadata?.component?.version !== releaseVersion || !Array.isArray(sbom.components) || !Array.isArray(sbom.dependencies)) process.exit(30);
const metadataText = ['BUILD-PROVENANCE.json', 'MANIFEST.json', 'SBOM.cdx.json', 'THIRD-PARTY-NOTICES.txt'].map((name) => fs.readFileSync(path.join(root, name), 'utf8')).join('\n');
if (/file:\/\//iu.test(metadataText) || /(?:^|["'\s])[A-Za-z]:[\\/]/u.test(metadataText)) process.exit(31);
NODE

(
  cd -- "$CANDIDATE_ROOT"
  sha256sum --check --strict SHA256SUMS
)

readonly REPACKED="$TEMP_ROOT/repacked.tar.gz"
(
  cd -- "$CANDIDATE_ROOT"
  "$TAR_BIN" --format=ustar --mtime="@$EXPECTED_SOURCE_DATE_EPOCH" --owner=0 --group=0 \
    --numeric-owner --mode='u+rwX,go+rX,go-w' -cf - "${EXPECTED_FILES[@]}" |
    "$GZIP_BIN" -n -9 > "$REPACKED"
)
cmp -- "$ARCHIVE_PATH" "$REPACKED" || die 'archive is not byte-for-byte deterministic'

printf 'linux package verification: PASS (20 files)\n'

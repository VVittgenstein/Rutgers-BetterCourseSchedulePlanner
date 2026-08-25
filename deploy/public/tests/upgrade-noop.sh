#!/usr/bin/env bash

# H3 discriminator: a same-release run of upgrade.sh must be a genuine
# no-op-plus-liveness-check. The pre-hardening implementation called
# daemon-reload, enable, and restart on that path; with a recording
# systemctl stub, ANY invocation is the failure. A second phase pins that
# the liveness check is real: a probe that never returns 200 must fail the
# run -- still without touching systemctl.
#
# Root-free and host-safe: everything lives under a temporary BCSP_ROOT
# with stubbed systemctl/curl/flock. Needs real symlink support (Linux, or
# Windows developer mode); reports SKIPPED otherwise so a platform without
# symlinks cannot silently count this gate as passed.

set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
OPS_DIR="$(cd -- "$SCRIPT_DIR/../ops" && pwd)"
DEPLOY_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"

TEST_TMP="$(mktemp -d)"
cleanup() {
  rm -rf -- "$TEST_TMP"
}
trap cleanup EXIT

ln -s "$TEST_TMP" "$TEST_TMP/symlink-probe" 2>/dev/null || true
if [[ ! -L "$TEST_TMP/symlink-probe" ]]; then
  printf 'upgrade-noop: SKIPPED (no symlink support on this platform)\n' >&2
  exit 0
fi
rm -f -- "$TEST_TMP/symlink-probe"

# --- a complete, valid fake candidate (the exact 21-file allowlist) --------

CANDIDATE="$TEST_TMP/candidate"
install -d -m 0755 "$CANDIDATE/bin"
printf '#!/usr/bin/env bash\nexit 0\n' > "$CANDIDATE/bin/bcsp-server"
chmod 0755 "$CANDIDATE/bin/bcsp-server"
for metadata in BUILD-PROVENANCE.json FRONTEND-CAPABILITIES.json LICENSE \
  MANIFEST.json SBOM.cdx.json SHA256SUMS THIRD-PARTY-NOTICES.txt VERSION; do
  printf 'fixture\n' > "$CANDIDATE/$metadata"
done
install -d -m 0755 "$CANDIDATE/systemd" "$CANDIDATE/caddy" "$CANDIDATE/config" \
  "$CANDIDATE/ops" "$CANDIDATE/docs"
install -m 0644 "$DEPLOY_DIR/systemd/bcsp.service" "$CANDIDATE/systemd/bcsp.service"
install -m 0644 "$DEPLOY_DIR/caddy/Caddyfile.example" "$CANDIDATE/caddy/Caddyfile.example"
install -m 0644 "$DEPLOY_DIR/config/bcsp.env.example" "$CANDIDATE/config/bcsp.env.example"
install -m 0644 "$DEPLOY_DIR/config/bcsp.env.schema.json" \
  "$CANDIDATE/config/bcsp.env.schema.json"
install -m 0644 "$DEPLOY_DIR/docs/operator-runbook.md" "$CANDIDATE/docs/operator-runbook.md"
for script in backup.sh install.sh lib.sh restore.sh rollback.sh upgrade.sh verify.sh; do
  install -m 0755 "$OPS_DIR/$script" "$CANDIDATE/ops/$script"
done

# --- stubs ------------------------------------------------------------------

STUB_DIR="$TEST_TMP/stubs"
install -d -m 0755 "$STUB_DIR"
SYSTEMCTL_LOG="$TEST_TMP/systemctl.log"
: > "$SYSTEMCTL_LOG"
cat > "$STUB_DIR/systemctl-stub" <<STUB
#!/usr/bin/env bash
printf '%s\n' "\$*" >> "$SYSTEMCTL_LOG"
exit 0
STUB
CURL_MODE_FILE="$TEST_TMP/curl-mode"
printf 'live\n' > "$CURL_MODE_FILE"
CURL_LOG="$TEST_TMP/curl.log"
: > "$CURL_LOG"
cat > "$STUB_DIR/curl-stub" <<STUB
#!/usr/bin/env bash
printf '%s\n' "\$*" >> "$CURL_LOG"
if [[ "\$(cat "$CURL_MODE_FILE")" == "live" ]]; then
  printf '200'
else
  printf '503'
fi
STUB
cat > "$STUB_DIR/flock-stub" <<'STUB'
#!/usr/bin/env bash
exit 0
STUB
chmod 0755 "$STUB_DIR/systemctl-stub" "$STUB_DIR/curl-stub" "$STUB_DIR/flock-stub"

# --- an already-installed same-release state under BCSP_ROOT ----------------

FAKE_ROOT="$TEST_TMP/root"
export BCSP_ROOT="$FAKE_ROOT"
export BCSP_SYSTEMCTL="$STUB_DIR/systemctl-stub"
export BCSP_CURL="$STUB_DIR/curl-stub"
export BCSP_FLOCK="$STUB_DIR/flock-stub"
export BCSP_SKIP_USER_MANAGEMENT=1
export BCSP_SKIP_OWNERSHIP=1
export BCSP_HEALTH_ATTEMPTS=2
export BCSP_HEALTH_DELAY_SECONDS=0

install -d -m 0755 "$FAKE_ROOT/opt/bcsp/releases" "$FAKE_ROOT/etc/systemd/system"
install -d -m 0750 "$FAKE_ROOT/etc/bcsp" "$FAKE_ROOT/var/lib/bcsp"
install -d -m 0700 "$FAKE_ROOT/var/backups/bcsp" "$FAKE_ROOT/var/backups/bcsp/.ops"
cp -R -- "$CANDIDATE" "$FAKE_ROOT/opt/bcsp/releases/fixture-v1"
ln -s -- "$FAKE_ROOT/opt/bcsp/releases/fixture-v1" "$FAKE_ROOT/opt/bcsp/current"
install -m 0644 "$DEPLOY_DIR/systemd/bcsp.service" \
  "$FAKE_ROOT/etc/systemd/system/bcsp.service"
printf 'BCSP_PUBLIC_ORIGIN=https://planner.invalid\n' > "$FAKE_ROOT/etc/bcsp/bcsp.env"
chmod 0640 "$FAKE_ROOT/etc/bcsp/bcsp.env"

# --- phase 1: same release, live service => success, ZERO systemctl calls ---

if ! bash "$OPS_DIR/upgrade.sh" fixture-v1 "$CANDIDATE" > "$TEST_TMP/phase1.out" 2>&1; then
  cat "$TEST_TMP/phase1.out" >&2
  printf 'upgrade-noop: the same-release run must succeed when the service is live\n' >&2
  exit 1
fi
if [[ -s "$SYSTEMCTL_LOG" ]]; then
  printf 'upgrade-noop: the same-release run touched systemctl:\n' >&2
  cat "$SYSTEMCTL_LOG" >&2
  exit 1
fi
if ! grep -q 'nothing to restart' "$TEST_TMP/phase1.out"; then
  cat "$TEST_TMP/phase1.out" >&2
  printf 'upgrade-noop: the same-release run must say it restarted nothing\n' >&2
  exit 1
fi
if ! [[ -s "$CURL_LOG" ]]; then
  printf 'upgrade-noop: the same-release run must still probe liveness\n' >&2
  exit 1
fi

# --- phase 2: same release, dead service => failure, STILL no systemctl -----

printf 'dead\n' > "$CURL_MODE_FILE"
if bash "$OPS_DIR/upgrade.sh" fixture-v1 "$CANDIDATE" > "$TEST_TMP/phase2.out" 2>&1; then
  printf 'upgrade-noop: a dead service must fail the same-release check\n' >&2
  exit 1
fi
if ! grep -q 'active release is not live' "$TEST_TMP/phase2.out"; then
  cat "$TEST_TMP/phase2.out" >&2
  printf 'upgrade-noop: the failure must name the liveness check\n' >&2
  exit 1
fi
if [[ -s "$SYSTEMCTL_LOG" ]]; then
  printf 'upgrade-noop: a failed liveness check must not try to restart its way out:\n' >&2
  cat "$SYSTEMCTL_LOG" >&2
  exit 1
fi

printf 'upgrade-noop: PASS (same-release upgrade is a liveness check with zero systemctl calls)\n'

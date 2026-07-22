#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
INSTALLER="$ROOT/scripts/install-miniboard-ipd.sh"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

assert_file() {
  [ -f "$1" ] || fail "expected file: $1"
}

assert_contains() {
  needle=$1
  file=$2
  grep -F "$needle" "$file" >/dev/null || fail "expected '$needle' in $file"
}

make_fixture() {
  target=$1
  fixture_dir=$2
  version=${3:-0.0.0}
  package_dir=$fixture_dir/package-$target

  mkdir -p "$package_dir"
  cat > "$package_dir/miniboard-ipd" <<BIN
#!/bin/sh
if [ "\${1:-}" = "--version" ] || [ "\${1:-}" = "version" ]; then
  echo "miniboard-ipd $version"
  exit 0
fi
echo "\$0 \$*" >> "\$INSTALL_LOG"
if [ "\${1:-}" = "install" ]; then
  echo "Service was installed but not started."
  echo "Start it manually after booting the target system."
fi
BIN
  chmod +x "$package_dir/miniboard-ipd"

  tar -C "$package_dir" -czf "$fixture_dir/miniboard-ipd-$target.tar.gz" miniboard-ipd
  (
    cd "$fixture_dir"
    sha256sum "miniboard-ipd-$target.tar.gz" > "miniboard-ipd-$target.tar.gz.sha256"
  )
}

make_fake_curl() {
  fakebin=$1
  fixture_dir=$2

  mkdir -p "$fakebin"
  cat > "$fakebin/curl" <<'CURL'
#!/bin/sh
set -eu

out=
url=
write_format=
while [ "$#" -gt 0 ]; do
  case "$1" in
    -o)
      out=$2
      shift 2
      ;;
    -w)
      write_format=$2
      shift 2
      ;;
    -*)
      shift
      ;;
    *)
      url=$1
      shift
      ;;
  esac
done

[ -n "$url" ] || {
  echo "fake curl requires url" >&2
  exit 2
}

echo "$url" >> "$CURL_LOG"

case "$url" in
  */releases/latest)
    tag=${FAKE_LATEST_TAG:-v0.0.0}
    if [ -n "$write_format" ]; then
      printf '%s\n' "https://github.com/kxn/msu2-ip-display/releases/tag/$tag"
    fi
    if [ -n "$out" ] && [ "$out" != "/dev/null" ]; then
      : > "$out"
    fi
    exit 0
    ;;
esac

[ -n "$out" ] || {
  echo "fake curl requires -o for asset download" >&2
  exit 2
}

base=${url##*/}
cp "$FIXTURE_DIR/$base" "$out"
CURL
  chmod +x "$fakebin/curl"
  FIXTURE_DIR=$fixture_dir
  export FIXTURE_DIR
}

make_fake_busy_cp() {
  fakebin=$1

  cat > "$fakebin/cp" <<'CP'
#!/bin/sh
set -eu

last=
for arg in "$@"; do
  last=$arg
done

if [ "${BUSY_TARGET:-}" = "$last" ]; then
  echo "Text file busy" >&2
  exit 26
fi

/bin/cp "$@"
CP
  chmod +x "$fakebin/cp"
}

run_in_temp() {
  tmp=${TMPDIR:-/tmp}/msu2-install-test-$$-$1
  rm -rf "$tmp"
  mkdir -p "$tmp"
  echo "$tmp"
}

test_installs_matching_arch_and_registers_service() {
  tmp=$(run_in_temp default)
  fixture_dir=$tmp/fixtures
  fakebin=$tmp/fakebin
  install_root=$tmp/root
  log=$tmp/install.log
  curl_log=$tmp/curl.log
  mkdir -p "$fixture_dir"
  make_fixture linux-amd64 "$fixture_dir"
  make_fake_curl "$fakebin" "$fixture_dir"
  : > "$log"
  : > "$curl_log"

  PATH="$fakebin:$PATH" \
  MSU2_INSTALL_ROOT="$install_root" \
  MSU2_INSTALLER_ARCH=x86_64 \
  FAKE_LATEST_TAG=v0.0.0 \
  MSU2_RELEASE_BASE=https://example.invalid/releases/latest/download \
  INSTALL_LOG="$log" \
  CURL_LOG="$curl_log" \
    sh "$INSTALLER" --interface eth0 --dhcp-fail-delay-seconds 45 > "$tmp/out"

  installed=$install_root/usr/local/bin/miniboard-ipd
  assert_file "$installed"
  assert_contains "miniboard-ipd-linux-amd64.tar.gz" "$curl_log"
  assert_contains "miniboard-ipd-linux-amd64.tar.gz.sha256" "$curl_log"
  assert_contains "$installed install --interface eth0 --dhcp-fail-delay-seconds 45" "$log"
  assert_contains "Service was installed but not started." "$tmp/out"
  assert_contains "Start it manually after booting the target system." "$tmp/out"
}

test_unflashed_option_is_passed_to_service_install() {
  tmp=$(run_in_temp unflashed)
  fixture_dir=$tmp/fixtures
  fakebin=$tmp/fakebin
  install_root=$tmp/root
  log=$tmp/install.log
  curl_log=$tmp/curl.log
  mkdir -p "$fixture_dir"
  make_fixture linux-amd64 "$fixture_dir"
  make_fake_curl "$fakebin" "$fixture_dir"
  : > "$log"
  : > "$curl_log"

  PATH="$fakebin:$PATH" \
  MSU2_INSTALL_ROOT="$install_root" \
  MSU2_INSTALLER_ARCH=x86_64 \
  FAKE_LATEST_TAG=v0.0.0 \
  MSU2_RELEASE_BASE=https://example.invalid/releases/latest/download \
  INSTALL_LOG="$log" \
  CURL_LOG="$curl_log" \
    sh "$INSTALLER" --unflashed --interface eth0 > "$tmp/out"

  assert_contains "$install_root/usr/local/bin/miniboard-ipd install --unflashed --interface eth0" "$log"
}

test_no_service_only_installs_binary() {
  tmp=$(run_in_temp no-service)
  fixture_dir=$tmp/fixtures
  fakebin=$tmp/fakebin
  install_root=$tmp/root
  log=$tmp/install.log
  curl_log=$tmp/curl.log
  mkdir -p "$fixture_dir"
  make_fixture linux-arm32 "$fixture_dir"
  make_fake_curl "$fakebin" "$fixture_dir"
  : > "$log"
  : > "$curl_log"

  PATH="$fakebin:$PATH" \
  MSU2_INSTALL_ROOT="$install_root" \
  MSU2_INSTALLER_ARCH=armv7l \
  FAKE_LATEST_TAG=v0.0.0 \
  MSU2_RELEASE_BASE=https://example.invalid/releases/latest/download \
  INSTALL_LOG="$log" \
  CURL_LOG="$curl_log" \
    sh "$INSTALLER" --no-service

  assert_file "$install_root/usr/local/bin/miniboard-ipd"
  assert_contains "miniboard-ipd-linux-arm32.tar.gz" "$curl_log"
  [ ! -s "$log" ] || fail "expected no service registration call"
}

test_replaces_busy_existing_binary_without_direct_copy_to_install_path() {
  tmp=$(run_in_temp busy-target)
  fixture_dir=$tmp/fixtures
  fakebin=$tmp/fakebin
  install_root=$tmp/root
  log=$tmp/install.log
  curl_log=$tmp/curl.log
  installed=$install_root/usr/local/bin/miniboard-ipd
  mkdir -p "$fixture_dir" "$(dirname "$installed")"
  make_fixture linux-amd64 "$fixture_dir"
  make_fake_curl "$fakebin" "$fixture_dir"
  make_fake_busy_cp "$fakebin"
  : > "$log"
  : > "$curl_log"
  echo "old binary" > "$installed"
  chmod +x "$installed"

  PATH="$fakebin:$PATH" \
  MSU2_INSTALL_ROOT="$install_root" \
  MSU2_INSTALLER_ARCH=x86_64 \
  FAKE_LATEST_TAG=v0.0.0 \
  MSU2_RELEASE_BASE=https://example.invalid/releases/latest/download \
  INSTALL_LOG="$log" \
  CURL_LOG="$curl_log" \
  BUSY_TARGET="$installed" \
    sh "$INSTALLER" --no-service

  assert_file "$installed"
  assert_contains "echo \"\$0 \$*\" >> \"\$INSTALL_LOG\"" "$installed"
}

test_upgrade_stops_existing_service_before_replacing_binary() {
  tmp=$(run_in_temp upgrade)
  fixture_dir=$tmp/fixtures
  fakebin=$tmp/fakebin
  install_root=$tmp/root
  log=$tmp/install.log
  curl_log=$tmp/curl.log
  installed=$install_root/usr/local/bin/miniboard-ipd
  mkdir -p "$fixture_dir" "$(dirname "$installed")"
  make_fixture linux-amd64 "$fixture_dir"
  make_fake_curl "$fakebin" "$fixture_dir"
  : > "$log"
  : > "$curl_log"
  cat > "$installed" <<'OLD'
#!/bin/sh
echo "old:$0 $*" >> "$INSTALL_LOG"
OLD
  chmod +x "$installed"

  PATH="$fakebin:$PATH" \
  MSU2_INSTALL_ROOT="$install_root" \
  MSU2_INSTALLER_ARCH=x86_64 \
  FAKE_LATEST_TAG=v0.0.0 \
  MSU2_RELEASE_BASE=https://example.invalid/releases/latest/download \
  INSTALL_LOG="$log" \
  CURL_LOG="$curl_log" \
    sh "$INSTALLER" --interface eth0 > "$tmp/out"

  assert_contains "old:$installed uninstall" "$log"
  assert_contains "$installed install --interface eth0" "$log"
  assert_contains "Service was installed but not started." "$tmp/out"
}

test_resolves_latest_release_and_verifies_installed_version() {
  tmp=$(run_in_temp latest-version)
  fixture_dir=$tmp/fixtures
  fakebin=$tmp/fakebin
  install_root=$tmp/root
  log=$tmp/install.log
  curl_log=$tmp/curl.log
  mkdir -p "$fixture_dir"
  make_fixture linux-amd64 "$fixture_dir" 9.8.7
  make_fake_curl "$fakebin" "$fixture_dir"
  : > "$log"
  : > "$curl_log"

  PATH="$fakebin:$PATH" \
  MSU2_INSTALL_ROOT="$install_root" \
  MSU2_INSTALLER_ARCH=x86_64 \
  FAKE_LATEST_TAG=v9.8.7 \
  INSTALL_LOG="$log" \
  CURL_LOG="$curl_log" \
    sh "$INSTALLER" --no-service > "$tmp/out"

  assert_contains "https://github.com/kxn/msu2-ip-display/releases/latest" "$curl_log"
  assert_contains "https://github.com/kxn/msu2-ip-display/releases/download/v9.8.7/miniboard-ipd-linux-amd64.tar.gz" "$curl_log"
  assert_contains "Resolved release v9.8.7" "$tmp/out"
  assert_contains "Installed version: miniboard-ipd 9.8.7" "$tmp/out"
}

test_installed_version_mismatch_fails() {
  tmp=$(run_in_temp version-mismatch)
  fixture_dir=$tmp/fixtures
  fakebin=$tmp/fakebin
  install_root=$tmp/root
  log=$tmp/install.log
  curl_log=$tmp/curl.log
  mkdir -p "$fixture_dir"
  make_fixture linux-amd64 "$fixture_dir" 9.8.6
  make_fake_curl "$fakebin" "$fixture_dir"
  : > "$log"
  : > "$curl_log"

  if PATH="$fakebin:$PATH" \
    MSU2_INSTALL_ROOT="$install_root" \
    MSU2_INSTALLER_ARCH=x86_64 \
    FAKE_LATEST_TAG=v9.8.7 \
    INSTALL_LOG="$log" \
    CURL_LOG="$curl_log" \
      sh "$INSTALLER" --no-service > "$tmp/out" 2> "$tmp/err"; then
    fail "version mismatch should fail"
  fi

  assert_contains "installed version mismatch" "$tmp/err"
}

test_unsupported_arch_fails_before_download() {
  tmp=$(run_in_temp unsupported)
  fakebin=$tmp/fakebin
  install_root=$tmp/root
  log=$tmp/install.log
  mkdir -p "$fakebin"
  : > "$log"
  cat > "$fakebin/curl" <<'CURL'
#!/bin/sh
echo "curl should not be called" >&2
exit 9
CURL
  chmod +x "$fakebin/curl"

  if PATH="$fakebin:$PATH" \
    MSU2_INSTALL_ROOT="$install_root" \
    MSU2_INSTALLER_ARCH=mips64 \
    INSTALL_LOG="$log" \
      sh "$INSTALLER" > "$tmp/out" 2> "$tmp/err"; then
    fail "unsupported arch should fail"
  fi

  assert_contains "unsupported architecture: mips64" "$tmp/err"
}

test_installs_matching_arch_and_registers_service
test_unflashed_option_is_passed_to_service_install
test_no_service_only_installs_binary
test_replaces_busy_existing_binary_without_direct_copy_to_install_path
test_upgrade_stops_existing_service_before_replacing_binary
test_resolves_latest_release_and_verifies_installed_version
test_installed_version_mismatch_fails
test_unsupported_arch_fails_before_download

echo "install-miniboard-ipd tests passed"

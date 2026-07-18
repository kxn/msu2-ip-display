#!/bin/sh
set -eu

REPO=${MSU2_REPO:-kxn/msu2-ip-display}
INSTALL_ROOT=${MSU2_INSTALL_ROOT:-}
INSTALL_PATH="$INSTALL_ROOT/usr/local/bin/miniboard-ipd"
INSTALL_SERVICE=1

usage() {
  cat <<'USAGE'
Install miniboard-ipd for the current Linux architecture.

Usage:
  curl -fsSL https://raw.githubusercontent.com/kxn/msu2-ip-display/master/scripts/install-miniboard-ipd.sh | sudo sh
  curl -fsSL https://raw.githubusercontent.com/kxn/msu2-ip-display/master/scripts/install-miniboard-ipd.sh | sudo sh -s -- --interface eth0
  curl -fsSL https://raw.githubusercontent.com/kxn/msu2-ip-display/master/scripts/install-miniboard-ipd.sh | sudo sh -s -- --no-service

Installer options:
  --no-service    Install /usr/local/bin/miniboard-ipd but do not run miniboard-ipd install.
  --help          Show this help.

Any remaining arguments are passed to: miniboard-ipd install ...
USAGE
}

die() {
  echo "install-miniboard-ipd: $*" >&2
  exit 1
}

need_command() {
  command -v "$1" >/dev/null 2>&1 || die "missing required command: $1"
}

binary_version() {
  if [ -x "$1" ]; then
    "$1" --version 2>/dev/null || true
  fi
}

resolve_release_tag() {
  if [ "${MSU2_RELEASE_TAG:-}" ]; then
    echo "$MSU2_RELEASE_TAG"
    return
  fi

  latest_url="https://github.com/$REPO/releases/latest"
  effective=$(curl -fsSLI -o /dev/null -w '%{url_effective}' "$latest_url") || die "failed to resolve latest release"
  tag=${effective##*/}
  case "$tag" in
    ""|latest)
      die "could not resolve latest release tag from $effective"
      ;;
  esac
  echo "$tag"
}

expected_version_for_tag() {
  tag=$1
  case "$tag" in
    v*) echo "${tag#v}" ;;
    *) echo "$tag" ;;
  esac
}

target_for_arch() {
  arch=$1
  case "$arch" in
    x86_64|amd64)
      echo linux-amd64
      ;;
    aarch64|arm64)
      echo linux-arm64
      ;;
    armv7l|armv7*|armhf)
      echo linux-arm32
      ;;
    *)
      die "unsupported architecture: $arch"
      ;;
  esac
}

make_tmp_dir() {
  base=${TMPDIR:-/tmp}
  mktemp -d "$base/msu2-ipd.XXXXXX"
}

cleanup() {
  if [ "${INSTALL_TMP:-}" ]; then
    rm -f "$INSTALL_TMP"
  fi
  if [ "${TMP_DIR:-}" ]; then
    rm -rf "$TMP_DIR"
  fi
}

stop_existing_service() {
  if [ "$INSTALL_SERVICE" = "1" ] && [ -x "$INSTALL_PATH" ]; then
    echo "Stopping existing service"
    "$INSTALL_PATH" uninstall >/dev/null 2>&1 || true
  fi
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --no-service)
      INSTALL_SERVICE=0
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    --)
      shift
      break
      ;;
    *)
      break
      ;;
  esac
done

if [ -z "$INSTALL_ROOT" ] && [ "$(id -u)" != "0" ]; then
  die "run as root, for example: curl -fsSL https://raw.githubusercontent.com/$REPO/master/scripts/install-miniboard-ipd.sh | sudo sh"
fi

need_command curl
need_command tar
need_command sha256sum
need_command mktemp

ARCH=${MSU2_INSTALLER_ARCH:-$(uname -m)}
TARGET=$(target_for_arch "$ARCH")
ASSET="miniboard-ipd-$TARGET.tar.gz"

RELEASE_TAG=$(resolve_release_tag)
EXPECTED_VERSION=$(expected_version_for_tag "$RELEASE_TAG")
RELEASE_BASE=${MSU2_RELEASE_BASE:-https://github.com/$REPO/releases/download/$RELEASE_TAG}

TMP_DIR=$(make_tmp_dir)
trap cleanup EXIT HUP INT TERM

ARCHIVE="$TMP_DIR/$ASSET"
CHECKSUM="$TMP_DIR/$ASSET.sha256"

echo "Resolved release $RELEASE_TAG"
echo "Downloading $ASSET"
curl -fsSL "$RELEASE_BASE/$ASSET" -o "$ARCHIVE"
curl -fsSL "$RELEASE_BASE/$ASSET.sha256" -o "$CHECKSUM"

(
  cd "$TMP_DIR"
  sha256sum -c "$ASSET.sha256" >/dev/null
) || die "checksum verification failed for $ASSET"

tar -xzf "$ARCHIVE" -C "$TMP_DIR"
[ -f "$TMP_DIR/miniboard-ipd" ] || die "archive did not contain miniboard-ipd"

OLD_VERSION=$(binary_version "$INSTALL_PATH")
if [ "$OLD_VERSION" ]; then
  echo "Existing version: $OLD_VERSION"
fi

stop_existing_service

INSTALL_DIR=$(dirname "$INSTALL_PATH")
mkdir -p "$INSTALL_DIR"
INSTALL_TMP="$INSTALL_DIR/.miniboard-ipd.$$"
cp "$TMP_DIR/miniboard-ipd" "$INSTALL_TMP"
chmod 0755 "$INSTALL_TMP"
mv -f "$INSTALL_TMP" "$INSTALL_PATH"
INSTALL_TMP=

echo "Installed $INSTALL_PATH"
INSTALLED_VERSION=$(binary_version "$INSTALL_PATH")
if [ "$INSTALLED_VERSION" ]; then
  echo "Installed version: $INSTALLED_VERSION"
else
  die "installed binary does not report a version"
fi

if [ "$INSTALLED_VERSION" != "miniboard-ipd $EXPECTED_VERSION" ]; then
  die "installed version mismatch: expected miniboard-ipd $EXPECTED_VERSION, got $INSTALLED_VERSION"
fi

if [ "$INSTALL_SERVICE" = "1" ]; then
  echo "Registering service"
  "$INSTALL_PATH" install "$@"
else
  echo "Skipped service registration"
fi

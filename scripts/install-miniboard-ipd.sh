#!/bin/sh
set -eu

REPO=${MSU2_REPO:-kxn/msu2-ip-display}
RELEASE_BASE=${MSU2_RELEASE_BASE:-https://github.com/$REPO/releases/latest/download}
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
  if [ "${TMP_DIR:-}" ]; then
    rm -rf "$TMP_DIR"
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

TMP_DIR=$(make_tmp_dir)
trap cleanup EXIT HUP INT TERM

ARCHIVE="$TMP_DIR/$ASSET"
CHECKSUM="$TMP_DIR/$ASSET.sha256"

echo "Downloading $ASSET"
curl -fsSL "$RELEASE_BASE/$ASSET" -o "$ARCHIVE"
curl -fsSL "$RELEASE_BASE/$ASSET.sha256" -o "$CHECKSUM"

(
  cd "$TMP_DIR"
  sha256sum -c "$ASSET.sha256" >/dev/null
) || die "checksum verification failed for $ASSET"

tar -xzf "$ARCHIVE" -C "$TMP_DIR"
[ -f "$TMP_DIR/miniboard-ipd" ] || die "archive did not contain miniboard-ipd"

mkdir -p "$(dirname "$INSTALL_PATH")"
cp "$TMP_DIR/miniboard-ipd" "$INSTALL_PATH"
chmod 0755 "$INSTALL_PATH"

echo "Installed $INSTALL_PATH"

if [ "$INSTALL_SERVICE" = "1" ]; then
  echo "Registering service"
  "$INSTALL_PATH" install "$@"
else
  echo "Skipped service registration"
fi

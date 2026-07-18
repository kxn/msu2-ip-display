# Release And Install

## GitHub Actions

`CI` runs on pushes to `master`, pull requests, and manual dispatch. It builds and tests:

- `miniboard-ipd` Linux host artifacts for `linux-amd64`, `linux-arm64`, and `linux-arm32`.
- MSU2 flasher artifacts for Windows, Linux, macOS x64, and macOS arm64.

`Release` runs on tags matching `v*` or manual dispatch with an existing tag. It builds the same artifacts and uploads them to the matching GitHub Release.

## Host Installer

The latest host daemon can be installed with:

```sh
curl -fsSL https://raw.githubusercontent.com/kxn/msu2-ip-display/master/scripts/install-miniboard-ipd.sh | sudo sh
```

To bind the installed service to a fixed network interface:

```sh
curl -fsSL https://raw.githubusercontent.com/kxn/msu2-ip-display/master/scripts/install-miniboard-ipd.sh | sudo sh -s -- --interface eth0
```

The installer:

1. Detects the current architecture with `uname -m`.
2. Downloads the matching `miniboard-ipd-linux-*.tar.gz` from the latest GitHub Release.
3. Downloads and verifies the matching `.sha256` file.
4. Installs the binary to `/usr/local/bin/miniboard-ipd`.
5. Runs `miniboard-ipd install "$@"` unless `--no-service` is passed.

Supported architecture mappings:

| `uname -m` | Release asset |
| --- | --- |
| `x86_64`, `amd64` | `miniboard-ipd-linux-amd64.tar.gz` |
| `aarch64`, `arm64` | `miniboard-ipd-linux-arm64.tar.gz` |
| `armv7l`, `armv7*`, `armhf` | `miniboard-ipd-linux-arm32.tar.gz` |

Use `--no-service` to install only the binary:

```sh
curl -fsSL https://raw.githubusercontent.com/kxn/msu2-ip-display/master/scripts/install-miniboard-ipd.sh | sudo sh -s -- --no-service
```

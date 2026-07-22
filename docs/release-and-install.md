# Release And Install

## GitHub Actions

`CI` runs on pushes to `master`, pull requests, and manual dispatch. It builds and tests:

- `miniboard-ipd` Linux host artifacts for `linux-amd64`, `linux-arm64`, and `linux-arm32`.
- MSU2 flasher portable artifacts:
  - `MSU2.Flasher-windows-x64.exe`
  - `MSU2.Flasher-linux-x64`
  - macOS x64 and arm64 `.dmg` images

`Release` runs on tags matching `v*` or manual dispatch with an existing tag. It builds the same artifacts and uploads them to the matching GitHub Release.

The flasher release intentionally does not publish Windows installers, Linux distro packages, or AppImage bundles. Windows and Linux releases are one portable executable per platform. On Linux, make the downloaded binary executable before running it:

```sh
chmod +x ./MSU2.Flasher-linux-x64
./MSU2.Flasher-linux-x64
```

On Windows, run `MSU2.Flasher-windows-x64.exe` directly. The portable Windows executable still requires the Microsoft WebView2 Runtime to exist on the system.

## Host Installer

The latest host daemon can be installed with:

```sh
curl -fsSL https://raw.githubusercontent.com/kxn/msu2-ip-display/master/scripts/install-miniboard-ipd.sh | sudo sh
```

To bind the installed service to a fixed network interface:

```sh
curl -fsSL https://raw.githubusercontent.com/kxn/msu2-ip-display/master/scripts/install-miniboard-ipd.sh | sudo sh -s -- --interface eth0
```

For field diagnostics, enable detailed daemon logging:

```sh
curl -fsSL https://raw.githubusercontent.com/kxn/msu2-ip-display/master/scripts/install-miniboard-ipd.sh | sudo sh -s -- --debug
```

On OpenRC systems, the generated service writes stdout/stderr to `/var/log/miniboard-ipd.log`.

The installer:

1. Detects the current architecture with `uname -m`.
2. Downloads the matching `miniboard-ipd-linux-*.tar.gz` from the latest GitHub Release.
3. Downloads and verifies the matching `.sha256` file.
4. Installs the binary to `/usr/local/bin/miniboard-ipd`.
5. Runs `miniboard-ipd install "$@"` unless `--no-service` is passed.
6. Verifies the installed binary version and prints the manual start command.

`miniboard-ipd install` installs service files and enables boot start when the init system supports it, but it does not start or restart the service. This keeps the installer usable when preparing an embedded root filesystem from a chroot. Start the service manually after booting the target system.

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

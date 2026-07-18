# Host USB

`miniboard-ipd` is the Linux host-side daemon for showing a headless device's IPv4 address on an MSU2 MINI USB screen.

Design:

- `docs/superpowers/specs/2026-07-17-host-usb-ip-display-design.md`

Examples:

```bash
miniboard-ipd run
miniboard-ipd run --interface eth0
miniboard-ipd run --debug
miniboard-ipd install --interface eth0 --dhcp-fail-delay-seconds 45
```

## One-line install

The installer downloads the latest GitHub Release asset matching the current Linux architecture, verifies its SHA-256 checksum, installs `miniboard-ipd` to `/usr/local/bin`, then runs `miniboard-ipd install ...`.

```bash
curl -fsSL https://raw.githubusercontent.com/kxn/msu2-ip-display/master/scripts/install-miniboard-ipd.sh | sudo sh
```

Pass daemon options after `sh -s --`; they are embedded into the generated service/init script:

```bash
curl -fsSL https://raw.githubusercontent.com/kxn/msu2-ip-display/master/scripts/install-miniboard-ipd.sh | sudo sh -s -- --interface eth0 --dhcp-fail-delay-seconds 45
```

Enable detailed daemon logging while troubleshooting:

```bash
curl -fsSL https://raw.githubusercontent.com/kxn/msu2-ip-display/master/scripts/install-miniboard-ipd.sh | sudo sh -s -- --debug
```

OpenRC services write stdout/stderr to `/var/log/miniboard-ipd.log`.

Install only the binary without registering a service:

```bash
curl -fsSL https://raw.githubusercontent.com/kxn/msu2-ip-display/master/scripts/install-miniboard-ipd.sh | sudo sh -s -- --no-service
```

## Build

```bash
cargo test
cargo build --release
```

## Commands

```bash
miniboard-ipd run
miniboard-ipd run --interface eth0
miniboard-ipd run --debug
miniboard-ipd install --interface eth0 --dhcp-fail-delay-seconds 45
miniboard-ipd uninstall
miniboard-ipd status
```

`install` stores command-line options, including `--debug`, in the generated service/init command. v1 does not read a config file.

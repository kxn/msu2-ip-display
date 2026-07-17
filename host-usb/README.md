# Host USB

`miniboard-ipd` is the Linux host-side daemon for showing a headless device's IPv4 address on an MSU2 MINI USB screen.

Design:

- `docs/superpowers/specs/2026-07-17-host-usb-ip-display-design.md`

Examples:

```bash
miniboard-ipd run
miniboard-ipd run --interface eth0
miniboard-ipd install --interface eth0 --dhcp-fail-delay-seconds 45
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
miniboard-ipd install --interface eth0 --dhcp-fail-delay-seconds 45
miniboard-ipd uninstall
miniboard-ipd status
```

`install` stores command-line options in the generated service/init command. v1 does not read a config file.

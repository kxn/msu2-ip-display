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

v1 has no config file. Options passed to `install` are embedded into the installed service/init script.

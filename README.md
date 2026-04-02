# x11idle

X11 idle daemon written in Rust.

`x11idle` monitors idle time via XScreenSaver, listens to systemd-logind D-Bus events, serves the `org.freedesktop.ScreenSaver` D-Bus API, and executes configurable commands on timeout and resume.

## Features

- XScreenSaver idle time tracking
- XInput2 physical input detection for reliable resume under screen lockers
- systemd-logind integration (Sleep, Lock, Unlock)
- `org.freedesktop.ScreenSaver` D-Bus service with inhibit support
- Configurable idle listeners with timeout and resume commands
- Inhibit-aware idle scheduling (pauses timer during video playback, etc.)
- Near-zero CPU usage (event-driven, sleeps between events)
- Debug mode via `--debug` flag or `X11IDLE_DEBUG` env var

## Installation

### Option 1: Installer script

```bash
curl -fsSL https://raw.githubusercontent.com/adanft/x11idle/main/install.sh | sudo bash
```

If you prefer to inspect the script before running it:

```bash
curl -O https://raw.githubusercontent.com/adanft/x11idle/main/install.sh
less install.sh
sudo bash install.sh
```

This installs the latest released binary to:

```bash
/usr/local/bin/x11idle
```

### Option 2: Manual install from release

Download the `x11idle` binary from GitHub Releases and copy it to `/usr/local/bin`:

```bash
chmod +x x11idle
sudo mv x11idle /usr/local/bin/x11idle
```

### Option 3: Build from source

```bash
git clone https://github.com/adanft/x11idle.git
cd x11idle
cargo build --release
sudo cp target/release/x11idle /usr/local/bin/x11idle
```

## Usage

```bash
x11idle
```

```bash
x11idle --debug
```

```
X11 idle daemon with D-Bus integration

Usage: x11idle [OPTIONS]

Options:
  -d, --debug    Enable verbose debug output [env: X11IDLE_DEBUG=]
  -h, --help     Show this help message
  -V, --version  Print version
```

## Configuration

Copy the example config to your config directory:

```bash
mkdir -p ~/.config/x11idle
cp assets/config.toml ~/.config/x11idle/config.toml
```

Config is loaded from:

1. `$XDG_CONFIG_HOME/x11idle/config.toml`
2. `~/.config/x11idle/config.toml`

If no config file is found, `x11idle` runs with an empty configuration.

All available options are fully documented in [`assets/config.toml`](assets/config.toml).

## Autostart

Add this to your autostart script or window manager configuration (e.g. `~/.xinitrc`, `~/.xprofile`, bspwmrc, i3 config, etc.):

```bash
# Disable X11 built-in screensaver and DPMS auto timeouts
# x11idle handles idle timeouts — let it be the only one in charge
xset s 0 0 noblank noexpose
xset dpms 0 0 0

# Start idle daemon (stop previous instance gracefully if running)
killall -q -TERM x11idle
x11idle &
```

## Requirements

- Linux with X11
- systemd-logind
- D-Bus (system and session bus)

## Release workflow

Build the release binary:

```bash
cargo build --release
```

The binary will be generated at:

```bash
target/release/x11idle
```

Upload that file as the release asset named exactly:

```bash
x11idle
```

This is required so `install.sh` can fetch it correctly.

## License

MIT

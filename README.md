# AuroraBox CLI

A command-line sing-box proxy manager with optional web UI, extracted from the [AuroraBox](https://github.com/OneOhCloud/AuroraBox) desktop application.

## Features

- **Proxy Engine**: Start/stop/reload sing-box with rule-based, global, or TUN mode
- **Config Generation**: Auto-generate sing-box `config.json` from templates + proxy servers
- **Database Management**: Add/remove/list proxy servers, subscriptions, and proxy groups
- **Web Server**: Optional REST API + embedded web UI (`--web` or `--web-server`)
- **Single Binary**: No external dependencies except sing-box itself

## Architecture

```
┌─────────────────────────────────────┐
│  CLI (clap)                         │
├─────────────────────────────────────┤
│  Service Layer (axum, optional)     │  ← REST API + embedded frontend
├─────────────────────────────────────┤
│  Proxy Layer (std::process)         │  ← sing-box process lifecycle
├─────────────────────────────────────┤
│  Core Layer (serde_json)            │  ← Config template merging
├─────────────────────────────────────┤
│  Database Layer (rusqlite/SQLite)   │  ← Proxy/subscription storage
└─────────────────────────────────────┘
```

Three layers, pluggable:
- **Core**: Generates sing-box config.json from templates + user data
- **Proxy**: Launches/monitors sing-box via `std::process`
- **Service**: Optional axum web server (only when `--web-server` flag)

## Installation

### From Source

```bash
git clone git@github.com:guaixian/aurorabox-cli.git
cd aurorabox-cli
cargo build --release
```

### Install sing-box

```bash
# Auto-download
aurorabox install

# Or manually place sing-box in PATH or ~/.local/share/aurorabox/bin/
```

## Quick Start

```bash
# Add a proxy server
aurorabox add proxy socks5 --name "My Proxy" --server 127.0.0.1 --port 1080
aurorabox add proxy ss --name "SS Node" --server example.com --port 8388 --password "mypass" --method aes-256-gcm

# List proxies
aurorabox list proxies

# Generate config (dry run)
aurorabox generate --mode rule

# Start the proxy engine
aurorabox start --mode rule

# Check status
aurorabox status

# Start with web UI
aurorabox start --mode rule --web-server --port 8080

# Or web-only
aurorabox web --port 8080

# Stop
aurorabox stop
```

## Commands

| Command | Description |
|---------|-------------|
| `start --mode rule\|global\|tun` | Start proxy engine |
| `stop` | Stop proxy engine |
| `reload` | Reload config (SIGHUP) |
| `status` | Show engine state |
| `add sub <url>` | Add subscription |
| `add proxy <proto>` | Add proxy server (ss/socks5/http/trojan/vless/hysteria2) |
| `list subscriptions\|proxies\|groups` | List stored items |
| `remove subscription\|proxy\|group <id>` | Remove item |
| `generate --mode rule\|global\|tun` | Generate config to stdout |
| `install [--version x.y.z]` | Download sing-box binary |
| `web --port 8080` | Start web server |

## Proxy Protocols

| Protocol | sing-box type | Example |
|----------|--------------|---------|
| `ss` | shadowsocks | `aurorabox add proxy ss --name ... --server ... --port ... --password ... --method aes-256-gcm` |
| `socks5` | socks | `aurorabox add proxy socks5 --name ... --server ... --port ...` |
| `http` | http | `aurorabox add proxy http --name ... --server ... --port ...` |
| `trojan` | trojan | `aurorabox add proxy trojan --name ... --server ... --port ... --password ...` |
| `vless` | vless | `aurorabox add proxy vless --name ... --server ... --port ... --uuid ...` |
| `hysteria2` | hysteria2 | `aurorabox add proxy hysteria2 --name ... --server ... --port ... --password ...` |

## Web API

When running with `--web-server`, the following REST API is available:

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/status` | Engine state |
| POST | `/api/start` | Start engine `{"mode": "rule"}` |
| POST | `/api/stop` | Stop engine |
| POST | `/api/reload` | Reload config |
| GET | `/api/subscriptions` | List subscriptions |
| POST | `/api/subscriptions` | Add subscription |
| GET | `/api/proxies` | List proxy servers |
| POST | `/api/proxies` | Add proxy server |
| GET | `/api/groups` | List proxy groups |
| POST | `/api/groups` | Add proxy group |
| GET | `/api/health` | Health check |

## Configuration

- Config directory: `~/.config/aurorabox/`
- Database: `~/.config/aurorabox/data.db`
- sing-box binary: auto-detected from PATH or `~/.local/share/aurorabox/bin/sing-box`
- Set `SING_BOX_PATH` environment variable to override sing-box location

## Build Options

```bash
# Full build with web server
cargo build --release

# Minimal build (no web server, smaller binary)
cargo build --release --no-default-features
```

## Database Compatibility

The SQLite database schema is identical to AuroraBox desktop. Data can be shared between the CLI and desktop versions.

## License

Same as AuroraBox.

# Kekkai (結界)

GPU-rendered NordVPN client. Replaces the NordVPN GUI while using the NordVPN service for all VPN operations.

## Features

- GPU-accelerated server map and UI via garasu (wgpu Metal/Vulkan)
- NordVPN API for server list, recommendations, account info
- `nordvpn` CLI integration for connection management
- Smart server selection (latency, load, country preference)
- NordLynx (WireGuard) and OpenVPN protocol support
- Kill switch management
- Hot-reloadable configuration via shikumi

## Architecture

| Module | Purpose |
|--------|---------|
| `api` | NordVPN REST API + CLI wrapper |
| `servers` | Server list, filtering, optimal selection |
| `connection` | Connect/disconnect lifecycle, status monitoring |
| `render` | GPU server map and UI via garasu |
| `config` | shikumi-based configuration |

## Dependencies

- **garasu** — GPU rendering engine
- **tsunagu** — daemon IPC (background connection monitor)
- **shikumi** — config discovery + hot-reload

## Build

```bash
cargo build
cargo run
cargo run -- connect us
cargo run -- status
cargo run -- servers --country us
```

## Configuration

`~/.config/kekkai/kekkai.yaml`

```yaml
connection:
  protocol: NordLynx
  preferred_country: us
  auto_connect: false
  killswitch: true
```

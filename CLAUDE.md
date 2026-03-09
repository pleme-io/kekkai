# Mamorigami (守紙) — GPU NordVPN Client

Crate: `mamorigami` | Binary: `kekkai` | Config app name: `kekkai`

GPU-rendered NordVPN client. Uses NordVPN's service (REST API + `nordvpn` CLI) for all
VPN operations. Provides a GPU server map visualization, smart server selection, and
MCP-driven VPN management.

## Build & Test

```bash
cargo build                            # compile
cargo test --lib                       # unit tests
cargo run                              # launch GUI
cargo run -- connect us                # connect to best US server
cargo run -- disconnect                # disconnect
cargo run -- status                    # show connection status
cargo run -- servers --country "Japan" # list Japanese servers
cargo run -- daemon                    # start connection monitor daemon
```

## Competitive Position

| Competitor | Stack | Our advantage |
|-----------|-------|---------------|
| **NordVPN official** | Electron/Qt | GPU-rendered, vim-modal, MCP-drivable, Rhai scriptable |
| **Mullvad VPN** | Rust GUI | NordVPN's 6000+ server network, threat protection |
| **Tailscale** | Go mesh | Consumer VPN (exit to internet), not mesh networking |
| **WireGuard tools** | wg-quick | Full GUI with server map, smart selection, MCP automation |

Unique value: GPU server map visualization, MCP automation for VPN management,
vim-modal navigation, and Rhai scripting for automated connection policies.

## Architecture

### Module Map

```
src/
  main.rs          ← CLI entry point (clap: open, connect, disconnect, status, servers, daemon)
  config.rs        ← KekkaiConfig via shikumi (api, connection, appearance sections)
  api.rs           ← NordVPN REST API client + `nordvpn` CLI wrapper
  servers.rs       ← Server model, filtering (country/city/protocol/load), best_server()
  connection.rs    ← VPN lifecycle: connect, disconnect, status, reconnect
  render.rs        ← GPU UI (TODO: madori integration)

  map/             ← (planned) Server map visualization
    mod.rs         ← WorldMap struct, projection, viewport
    geo.rs         ← Country/city coordinates, server clustering
    render.rs      ← GPU map rendering (nodes, connections, labels)

  latency/         ← (planned) Latency measurement
    mod.rs         ← LatencyProber: async ICMP/TCP ping to server IPs
    cache.rs       ← Latency cache with TTL

  stats/           ← (planned) Connection statistics
    mod.rs         ← Bandwidth, uptime, transfer counters
    history.rs     ← Connection history (SQLite)

  mcp/             ← (planned) MCP server via kaname
    mod.rs         ← KekkaiMcp server struct
    tools.rs       ← Tool implementations

  scripting/       ← (planned) Rhai scripting via soushi
    mod.rs         ← Engine setup, kekkai.* API registration

module/
  default.nix      ← HM module (blackmatter.components.kekkai)
```

### Data Flow

```
NordVPN REST API ──────────────▸ Server[]
  (server list, recommendations)      │
                                      ▼
                             ServerFilter (country, city, protocol, max_load)
                                      │
                                      ▼
                              best_server() → Server
                                      │
                       ┌──────────────┴──────────────┐
                       ▼                              ▼
              `nordvpn` CLI                    GPU Map Render
         (connect/disconnect/status)      (server nodes, connection line)
                       │
                       ▼
              ConnectionStatus
         (connected/disconnected, IP, server, protocol, transfer stats)
```

### VPN Backend Architecture

**NordVPN REST API** (via todoku, currently raw reqwest):
- `GET /v1/servers` — full server list with load, technologies, location
- `GET /v1/servers/recommendations` — recommended servers for country/protocol
- `GET /v1/users/me` — account info, subscription status
- Server data is JSON, cached in memory with TTL

**`nordvpn` CLI** (subprocess):
- `nordvpn connect [country] [city]` — connect to VPN
- `nordvpn disconnect` — disconnect
- `nordvpn status` — connection status
- `nordvpn settings` — current settings (protocol, killswitch, DNS)
- `nordvpn set protocol NordLynx|OpenVPN` — change protocol

The split: API for data (server lists, account), CLI for operations (connect/disconnect).

### Current Implementation Status

**Done:**
- `servers.rs` — Server model, ServerFilter, `filter_servers()`, `best_server()`, 11 tests
- `config.rs` — shikumi integration with api/connection/appearance sections
- `api.rs` — NordVPN REST client + CLI wrapper (basic structure)
- `connection.rs` — Connection lifecycle management (basic structure)
- `main.rs` — CLI with connect/disconnect/status/servers subcommands

**Not started:**
- GUI rendering via madori/garasu/egaku
- Server map visualization (GPU-rendered world map)
- Latency measurement and probing
- Connection statistics and history
- MCP server via kaname
- Rhai scripting via soushi
- Daemon mode (connection monitor) via tsunagu
- HM module (module/default.nix)

## Configuration

Uses **shikumi** for config discovery and hot-reload:
- Config file: `~/.config/kekkai/kekkai.yaml`
- Env override: `$KEKKAI_CONFIG`
- Env prefix: `KEKKAI_` (e.g., `KEKKAI_CONNECTION__PROTOCOL=nordlynx`)
- Hot-reload on file change (nix-darwin symlink aware)

### Config Schema

```yaml
api:
  server_list_url: "https://api.nordvpn.com/v1/servers"
  cache_ttl_secs: 3600                           # server list cache TTL

connection:
  protocol: "nordlynx"                           # nordlynx | openvpn_udp | openvpn_tcp
  auto_connect: false                            # connect on launch
  preferred_country: null                        # default country
  preferred_city: null                           # default city
  kill_switch: true                              # enable kill switch
  dns: []                                        # custom DNS servers
  nordvpn_path: "nordvpn"                        # path to nordvpn CLI

appearance:
  width: 1200
  height: 800
  background: "#2e3440"
  foreground: "#eceff4"
  accent: "#88c0d0"
  map_style: "dark"                              # dark | light | satellite

favorites:                                       # pinned servers
  - "us100.nordvpn.com"
  - "de50.nordvpn.com"
```

## Shared Library Integration

| Library | Usage |
|---------|-------|
| **shikumi** | Config discovery + hot-reload (`KekkaiConfig`) |
| **garasu** | GPU rendering for server map and UI |
| **madori** | App framework (event loop, render loop) |
| **egaku** | Widgets (server list, map viewport, status bar, connection panel) |
| **irodzuki** | Theme: base16 to GPU uniforms for map and UI |
| **todoku** | HTTP client for NordVPN REST API (replaces raw reqwest) |
| **tsunagu** | Daemon mode for connection monitor |
| **kaname** | MCP server framework |
| **soushi** | Rhai scripting engine |
| **awase** | Hotkey system for vim-modal navigation |
| **tsuuchi** | Notifications (connection state changes, kill switch triggers) |

**Note:** Cargo.toml currently references `kotoba` — this is the old name for `kaname`.
Update when crate is renamed.

## MCP Server (kaname)

Standard tools: `status`, `config_get`, `config_set`, `version`

App-specific tools:
- `connect(country?, city?, protocol?)` — connect to VPN
- `disconnect()` — disconnect
- `status()` — connection status (server, IP, protocol, uptime, transfer)
- `list_servers(country?, protocol?, max_load?)` — filtered server list
- `recommend_server(country?)` — best server recommendation
- `set_country(country)` — set preferred country and connect
- `get_ip()` — current public IP address
- `speed_test()` — bandwidth test on current connection
- `kill_switch(enabled)` — toggle kill switch
- `meshnet_status()` — NordVPN Meshnet status

## Rhai Scripting (soushi)

Scripts from `~/.config/kekkai/scripts/*.rhai`

```rhai
// Available API:
kekkai.connect("us")               // connect to best US server
kekkai.connect_city("us", "nyc")   // connect to NYC
kekkai.disconnect()                // disconnect
kekkai.status()                    // -> {connected, server, ip, protocol, uptime}
kekkai.servers("japan")            // -> [{hostname, country, city, load}]
kekkai.recommend()                 // -> best server based on config
kekkai.ip()                        // -> current public IP
kekkai.speed_test()                // -> {download_mbps, upload_mbps, latency_ms}
kekkai.auto_connect(true)          // toggle auto-connect
kekkai.protocol("nordlynx")       // set protocol
```

Event hooks: `on_startup`, `on_shutdown`, `on_connect(server)`, `on_disconnect`,
`on_reconnect(old_server, new_server)`

Example: auto-connect to fastest server on startup:
```rhai
fn on_startup() {
    let best = kekkai.recommend();
    kekkai.connect(best.country);
}
```

## Hotkey System (awase)

### Modes

**Normal** (default — main view):
| Key | Action |
|-----|--------|
| `c` | Quick connect (preferred or recommended server) |
| `d` | Disconnect |
| `s` | Show status panel |
| `m` | Switch to map view |
| `l` | Switch to server list view |
| `f` | Toggle favorites |
| `q` | Quit |
| `:` | Enter command mode |

**Map** (server map view):
| Key | Action |
|-----|--------|
| `h/j/k/l` | Pan map |
| `+/-` | Zoom in/out |
| `Enter` | Connect to highlighted server |
| `/` | Search country/city |
| `f` | Toggle favorite for highlighted server |
| `Esc` | Back to normal |

**List** (server list view):
| Key | Action |
|-----|--------|
| `j/k` | Navigate servers |
| `Enter` | Connect to selected server |
| `f` | Toggle favorite |
| `s` | Cycle sort (load, latency, name, country) |
| `/` | Filter servers |
| `Esc` | Back to normal |

**Command** (`:` prefix):
- `:connect <country> [city]` — connect to server
- `:disconnect` — disconnect
- `:protocol nordlynx|openvpn` — set protocol
- `:killswitch on|off` — toggle kill switch
- `:favorites` — show favorite servers
- `:dns <server>` — set custom DNS

## Nix Integration

### Flake Exports
- `packages.aarch64-darwin.{kekkai, default}` — the binary
- `overlays.default` — `pkgs.kekkai`
- `homeManagerModules.default` — `blackmatter.components.kekkai`
- `devShells.aarch64-darwin.default` — dev environment

### HM Module (planned)

Namespace: `blackmatter.components.kekkai`

Typed options:
- `enable` — install package + generate config
- `package` — override package
- `connection.{protocol, auto_connect, preferred_country, kill_switch}` — VPN settings
- `appearance.{width, height, background, foreground, accent, map_style}` — UI
- `favorites` — list of pinned server hostnames
- `daemon.enable` — connection monitor daemon (launchd/systemd)
- `mcp.enable` — register kekkai MCP server for Claude Code
- `extraSettings` — raw attrset escape hatch

YAML generated via `lib.generators.toYAML` -> `xdg.configFile."kekkai/kekkai.yaml"`

## GPU Server Map Design

The map view is the signature feature. Design guidance:

### Rendering Pipeline
1. **Base map** — simplified country boundaries as GPU-rendered polygons (garasu)
2. **Server nodes** — circles at city coordinates, size = capacity, color = load
   - Green (<30% load), Yellow (30-60%), Orange (60-80%), Red (>80%)
3. **Connection line** — animated arc from user location to connected server
4. **Labels** — country/city names via garasu text renderer (glyphon)
5. **Viewport** — pan/zoom with smooth animation, focus on connection target

### Coordinate System
- GeoJSON-like country boundaries stored as embedded data (simplified TopoJSON)
- Mercator projection for display, equirectangular for data storage
- Server coordinates from NordVPN API (lat/lon per city)

### Performance
- Batch all nodes into a single draw call (instanced rendering)
- Update node colors/sizes only when server list refreshes (not every frame)
- Map geometry is static — only nodes and connection line are dynamic

## Design Constraints

- **NordVPN CLI for operations** — never implement VPN tunnel directly, always delegate to `nordvpn` binary
- **NordVPN API for data** — server lists, recommendations, account info via REST
- **NordLynx preferred** — WireGuard-based protocol as default
- **Kill switch respect** — never override kill switch settings without explicit user action
- **No credentials storage** — NordVPN CLI handles auth, we never store NordVPN credentials
- **Latency is advisory** — show measured latency but do not make it the sole selection criterion
- **Map is optional** — server list view must be fully functional without map rendering

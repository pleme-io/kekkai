//! GPU rendering module for VPN client UI.
//!
//! Uses madori (app framework) + garasu (GPU primitives) + egaku (widgets).
//!
//! ## Views
//!
//! - **Status**: Connection state, server info, protocol, IP, uptime
//! - **Server List**: Scrollable server table with load indicators, country grouping
//! - **Search**: Filter servers by country/city name
//!
//! ## Rendering flow
//!
//! 1. madori handles window + event loop + frame timing
//! 2. Our `RenderCallback` implementation renders:
//!    - Background (Nord polar night)
//!    - Connection status bar (green connected / red disconnected)
//!    - Server list with load color coding
//!    - Search overlay when active
//!    - Text via garasu `TextRenderer`
//! 3. Input events dispatched to focused widget

use crate::config::AppearanceConfig;
use crate::connection::ConnectionState;
use crate::servers::Server;
use egaku::{ListView, TextInput};
use garasu::GpuContext;

/// Current view mode for the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewMode {
    /// Showing connection status (default view).
    Status,
    /// Showing server list.
    ServerList,
    /// Text-based server map: country -> city -> servers grouped view.
    ServerMap,
    /// Search overlay active.
    Search,
}

/// Sort mode for the server list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    /// Sort by load (lowest first).
    Load,
    /// Sort by country name.
    Country,
    /// Sort by server name.
    Name,
}

impl SortMode {
    /// Cycle to the next sort mode.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Load => Self::Country,
            Self::Country => Self::Name,
            Self::Name => Self::Load,
        }
    }

    /// Human-readable label for the sort mode.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Load => "Load",
            Self::Country => "Country",
            Self::Name => "Name",
        }
    }
}

/// A group of servers under a country -> city hierarchy.
#[derive(Debug, Clone)]
pub struct ServerGroup {
    /// Country name.
    pub country: String,
    /// Cities in this country, each with their servers.
    pub cities: Vec<CityGroup>,
}

/// Servers within a city.
#[derive(Debug, Clone)]
pub struct CityGroup {
    /// City name.
    pub city: String,
    /// Servers in this city.
    pub servers: Vec<Server>,
    /// Average load across servers in this city.
    pub avg_load: u8,
}

/// Application state for the VPN client UI.
pub struct KekkaiState {
    /// Current view mode.
    pub mode: ViewMode,
    /// Current VPN connection state.
    pub connection: ConnectionState,
    /// All loaded servers.
    pub servers: Vec<Server>,
    /// Filtered/sorted server list for display.
    pub display_servers: Vec<Server>,
    /// Grouped server view (country -> city -> servers).
    pub server_groups: Vec<ServerGroup>,
    /// Server map list widget (for grouped view).
    pub map_list: ListView,
    /// Server list widget.
    pub server_list: ListView,
    /// Search input widget.
    pub search_input: TextInput,
    /// Current sort mode.
    pub sort_mode: SortMode,
    /// Favorite server hostnames.
    pub favorites: Vec<String>,
    /// Whether to show only favorites.
    pub favorites_only: bool,
    /// Current protocol preference.
    pub protocol: String,
    /// Status message (e.g. "Connected to us100").
    pub status_message: Option<String>,
    /// Width of the window.
    pub width: u32,
    /// Height of the window.
    pub height: u32,
}

impl KekkaiState {
    /// Create a new empty state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            mode: ViewMode::Status,
            connection: ConnectionState::Disconnected,
            servers: Vec::new(),
            display_servers: Vec::new(),
            server_groups: Vec::new(),
            map_list: ListView::new(Vec::new(), 20),
            server_list: ListView::new(Vec::new(), 20),
            search_input: TextInput::new(),
            sort_mode: SortMode::Load,
            favorites: Vec::new(),
            favorites_only: false,
            protocol: "NordLynx".into(),
            status_message: None,
            width: 1200,
            height: 800,
        }
    }

    /// Set the server list and refresh display.
    pub fn set_servers(&mut self, servers: Vec<Server>) {
        self.servers = servers;
        self.refresh_display();
    }

    /// Refresh the display server list based on current filters and sort.
    pub fn refresh_display(&mut self) {
        let mut servers = self.servers.clone();

        // Apply favorites filter
        if self.favorites_only {
            servers.retain(|s| self.favorites.contains(&s.hostname));
        }

        // Apply search filter
        let query = self.search_input.text().to_lowercase();
        if !query.is_empty() {
            servers.retain(|s| {
                s.country.to_lowercase().contains(&query)
                    || s.city.to_lowercase().contains(&query)
                    || s.hostname.to_lowercase().contains(&query)
            });
        }

        // Sort
        match self.sort_mode {
            SortMode::Load => servers.sort_by_key(|s| s.load),
            SortMode::Country => servers.sort_by(|a, b| {
                a.country.cmp(&b.country).then(a.city.cmp(&b.city))
            }),
            SortMode::Name => servers.sort_by(|a, b| a.hostname.cmp(&b.hostname)),
        }

        let display: Vec<String> = servers
            .iter()
            .map(|s| {
                let fav = if self.favorites.contains(&s.hostname) {
                    "* "
                } else {
                    "  "
                };
                let load_indicator = load_bar(s.load);
                format!(
                    "{fav}{} ({}, {}) {load_indicator} {}%",
                    s.hostname, s.country, s.city, s.load
                )
            })
            .collect();
        self.server_list.set_items(display);
        self.display_servers = servers.clone();

        // Build grouped view (country -> city -> servers)
        self.server_groups = build_server_groups(&servers, &self.favorites);
        let map_lines: Vec<String> = self.server_groups.iter().flat_map(|group| {
            let mut lines = Vec::new();
            let total: usize = group.cities.iter().map(|c| c.servers.len()).sum();
            lines.push(format!("{} ({total} servers)", group.country));
            for city in &group.cities {
                let fav_count = city.servers.iter()
                    .filter(|s| self.favorites.contains(&s.hostname))
                    .count();
                let fav_marker = if fav_count > 0 { " *" } else { "" };
                lines.push(format!(
                    "  {} — {} servers, avg {}% load{}",
                    city.city, city.servers.len(), city.avg_load, fav_marker
                ));
            }
            lines
        }).collect();
        self.map_list.set_items(map_lines);
    }

    /// Navigate down in the server list.
    pub fn move_down(&mut self) {
        self.server_list.select_next();
    }

    /// Navigate up in the server list.
    pub fn move_up(&mut self) {
        self.server_list.select_prev();
    }

    /// Get the currently selected server.
    #[must_use]
    pub fn selected_server(&self) -> Option<&Server> {
        let idx = self.server_list.selected_index();
        self.display_servers.get(idx)
    }

    /// Toggle favorite status for the selected server.
    pub fn toggle_favorite(&mut self) {
        if let Some(server) = self.selected_server().cloned() {
            if let Some(pos) = self.favorites.iter().position(|h| *h == server.hostname) {
                self.favorites.remove(pos);
            } else {
                self.favorites.push(server.hostname);
            }
            self.refresh_display();
        }
    }

    /// Enter search mode.
    pub fn enter_search(&mut self) {
        self.mode = ViewMode::Search;
        self.search_input = TextInput::new();
    }

    /// Apply search filter and refresh display.
    pub fn apply_search(&mut self) {
        self.refresh_display();
    }

    /// Go back from current mode.
    pub fn go_back(&mut self) {
        match self.mode {
            ViewMode::Search => {
                self.mode = ViewMode::ServerList;
                self.search_input = TextInput::new();
                self.refresh_display();
            }
            ViewMode::ServerList | ViewMode::ServerMap => {
                self.mode = ViewMode::Status;
            }
            ViewMode::Status => {
                // Already at top level
            }
        }
    }

    /// Cycle protocol preference.
    pub fn cycle_protocol(&mut self) {
        self.protocol = match self.protocol.as_str() {
            "NordLynx" => "OpenVPN UDP".into(),
            "OpenVPN UDP" => "OpenVPN TCP".into(),
            "OpenVPN TCP" => "NordLynx".into(),
            _ => "NordLynx".into(),
        };
    }

    /// Navigate down in the map list.
    pub fn map_move_down(&mut self) {
        self.map_list.select_next();
    }

    /// Navigate up in the map list.
    pub fn map_move_up(&mut self) {
        self.map_list.select_prev();
    }

    /// Recommend the best server based on current filters and favorites.
    #[must_use]
    pub fn recommended_server(&self) -> Option<&Server> {
        // Prefer favorites with low load, then any server with low load
        let fav_best = self.display_servers.iter()
            .filter(|s| self.favorites.contains(&s.hostname))
            .min_by_key(|s| s.load);

        if let Some(best) = fav_best {
            if best.load < 70 {
                return Some(best);
            }
        }

        self.display_servers.iter().min_by_key(|s| s.load)
    }

    /// Set a temporary status message.
    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = Some(msg.into());
    }

    /// Update connection state.
    pub fn set_connection(&mut self, state: ConnectionState) {
        self.connection = state;
    }
}

impl Default for KekkaiState {
    fn default() -> Self {
        Self::new()
    }
}

/// Build server groups from a flat list (country -> city -> servers).
fn build_server_groups(servers: &[Server], favorites: &[String]) -> Vec<ServerGroup> {
    use std::collections::BTreeMap;

    let mut countries: BTreeMap<String, BTreeMap<String, Vec<Server>>> = BTreeMap::new();

    for server in servers {
        countries
            .entry(server.country.clone())
            .or_default()
            .entry(server.city.clone())
            .or_default()
            .push(server.clone());
    }

    // Sort: countries with favorites first, then alphabetical
    let mut groups: Vec<ServerGroup> = countries
        .into_iter()
        .map(|(country, cities)| {
            let city_groups: Vec<CityGroup> = cities
                .into_iter()
                .map(|(city, mut servers_in_city)| {
                    servers_in_city.sort_by_key(|s| s.load);
                    let total_load: u32 = servers_in_city.iter().map(|s| u32::from(s.load)).sum();
                    let avg_load = if servers_in_city.is_empty() {
                        0
                    } else {
                        (total_load / servers_in_city.len() as u32) as u8
                    };
                    CityGroup {
                        city,
                        servers: servers_in_city,
                        avg_load,
                    }
                })
                .collect();

            ServerGroup { country, cities: city_groups }
        })
        .collect();

    // Put countries containing favorited servers first
    groups.sort_by(|a, b| {
        let a_has_fav = a.cities.iter().any(|c| {
            c.servers.iter().any(|s| favorites.contains(&s.hostname))
        });
        let b_has_fav = b.cities.iter().any(|c| {
            c.servers.iter().any(|s| favorites.contains(&s.hostname))
        });
        b_has_fav.cmp(&a_has_fav).then(a.country.cmp(&b.country))
    });

    groups
}

/// Simple text-based load indicator.
fn load_bar(load: u8) -> &'static str {
    match load {
        0..=19 => "[====      ]",
        20..=39 => "[=====     ]",
        40..=59 => "[======    ]",
        60..=79 => "[========  ]",
        _ => "[==========]",
    }
}

/// Collect lines to render based on current view mode.
fn collect_lines(state: &KekkaiState) -> Vec<(String, bool, bool)> {
    // Returns: (text, is_selected, is_accent)
    let mut lines: Vec<(String, bool, bool)> = Vec::new();

    // Connection status header (always shown)
    let conn_label = match &state.connection {
        ConnectionState::Disconnected => "Disconnected".to_string(),
        ConnectionState::Connecting => "Connecting...".to_string(),
        ConnectionState::Disconnecting => "Disconnecting...".to_string(),
        ConnectionState::Connected(info) => {
            format!(
                "Connected to {} ({}, {}) via {} — IP: {}",
                info.server, info.country, info.city, info.protocol, info.ip
            )
        }
    };
    let is_connected = state.connection.is_connected();

    match &state.mode {
        ViewMode::Status => {
            lines.push(("Kekkai — NordVPN Client".into(), false, true));
            lines.push((String::new(), false, false));
            lines.push((conn_label, is_connected, !is_connected));
            lines.push((String::new(), false, false));

            if let ConnectionState::Connected(info) = &state.connection {
                lines.push((format!("  Server:   {}", info.server), false, false));
                lines.push((format!("  Protocol: {}", info.protocol), false, false));
                lines.push((format!("  IP:       {}", info.ip), false, false));
                lines.push((format!("  Country:  {}", info.country), false, false));
                lines.push((format!("  City:     {}", info.city), false, false));
                if let Some(uptime) = info.uptime() {
                    lines.push((format!("  Uptime:   {}s", uptime.as_secs()), false, false));
                }
            }

            lines.push((String::new(), false, false));
            lines.push((format!("  Protocol: {}", state.protocol), false, false));
            if !state.favorites.is_empty() {
                lines.push((format!("  Favorites: {}", state.favorites.len()), false, false));
            }
            if let Some(best) = state.recommended_server() {
                lines.push((format!("  Recommended: {} ({}% load)", best.hostname, best.load), false, false));
            }

            lines.push((String::new(), false, false));
            lines.push((
                "[c] connect  [d] disconnect  [l] server list  [m] server map  [p] protocol  [Tab] switch view  [q] quit".into(),
                false,
                true,
            ));
        }
        ViewMode::ServerMap => {
            lines.push(("Server Map — Country / City".into(), false, true));

            // Mini status bar
            lines.push((format!("  Status: {conn_label}  |  Protocol: {}", state.protocol), is_connected, false));
            lines.push((String::new(), false, false));

            // Grouped server map
            for (i, item) in state.map_list.visible_items().iter().enumerate() {
                let real_idx = state.map_list.offset() + i;
                let selected = real_idx == state.map_list.selected_index();
                let prefix = if selected { "> " } else { "  " };
                lines.push((format!("{prefix}{item}"), selected, false));
            }

            if state.map_list.is_empty() {
                lines.push(("  (no servers loaded)".into(), false, true));
            }

            lines.push((String::new(), false, false));
            lines.push((
                "[j/k] navigate  [f] favorites only  [Esc] back  [Tab] switch view  [q] quit".into(),
                false,
                true,
            ));
        }
        ViewMode::ServerList | ViewMode::Search => {
            let title = if state.mode == ViewMode::Search {
                format!("Search: {}", state.search_input.text())
            } else {
                format!(
                    "Servers — sorted by {} ({} total)  |  Protocol: {}",
                    state.sort_mode.label(),
                    state.display_servers.len(),
                    state.protocol
                )
            };
            lines.push((title, false, true));

            // Mini status bar
            lines.push((format!("  Status: {conn_label}"), is_connected, false));
            lines.push((String::new(), false, false));

            // Server list
            for (i, item) in state.server_list.visible_items().iter().enumerate() {
                let real_idx = state.server_list.offset() + i;
                let selected = real_idx == state.server_list.selected_index();
                let prefix = if selected { "> " } else { "  " };
                lines.push((format!("{prefix}{item}"), selected, false));
            }

            if state.server_list.is_empty() {
                lines.push(("  (no servers)".into(), false, true));
            }

            lines.push((String::new(), false, false));
            let help = if state.mode == ViewMode::Search {
                "[type] filter  [Enter] select  [Esc] cancel"
            } else {
                "[j/k] navigate  [Enter] connect  [f] favorite  [s] sort  [/] search  [p] protocol  [d] disconnect  [Esc] back  [q] quit"
            };
            lines.push((help.into(), false, true));
        }
    }

    if let Some(ref msg) = state.status_message {
        lines.push((String::new(), false, false));
        lines.push((msg.clone(), false, true));
    }

    lines
}

/// GPU renderer for the kekkai VPN client.
pub struct KekkaiRenderer {
    /// Application state.
    pub state: KekkaiState,
    /// Background clear color.
    bg_color: wgpu::Color,
    /// Font size in pixels.
    font_size: f32,
    /// Line height in pixels.
    line_height: f32,
}

impl KekkaiRenderer {
    /// Create a new renderer with the given appearance config.
    #[must_use]
    pub fn new(appearance: &AppearanceConfig) -> Self {
        let bg = egaku::theme::hex_to_rgba(&appearance.background)
            .unwrap_or([0.180, 0.204, 0.251, 1.0]);

        Self {
            state: KekkaiState::new(),
            bg_color: wgpu::Color {
                r: f64::from(bg[0]),
                g: f64::from(bg[1]),
                b: f64::from(bg[2]),
                a: f64::from(bg[3]),
            },
            font_size: 16.0,
            line_height: 24.0,
        }
    }
}

impl madori::RenderCallback for KekkaiRenderer {
    fn init(&mut self, _gpu: &GpuContext) {
        tracing::info!("kekkai GPU renderer initialized");
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.state.width = width;
        self.state.height = height;
    }

    fn render(&mut self, ctx: &mut madori::RenderContext<'_>) {
        let mut encoder = ctx.gpu.device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor {
                label: Some("kekkai_render"),
            },
        );

        // Pass 1: clear background
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("kekkai_clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: ctx.surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.bg_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        // Pass 2: text rendering
        let lines = collect_lines(&self.state);
        let padding = 12.0_f32;

        let normal_color = glyphon::Color::rgba(236, 239, 244, 255);
        let accent_color = glyphon::Color::rgba(136, 192, 208, 255);
        let connected_color = glyphon::Color::rgba(163, 190, 140, 255); // Nord green

        let mut buffers = Vec::new();
        for (text, selected, is_accent) in &lines {
            let color = if *selected {
                connected_color
            } else if *is_accent {
                accent_color
            } else {
                normal_color
            };
            let attrs = glyphon::Attrs::new().color(color);
            let mut buf = ctx.text.create_buffer(text, self.font_size, self.line_height);
            buf.set_text(
                &mut ctx.text.font_system,
                text,
                &attrs,
                glyphon::Shaping::Advanced,
            );
            buf.shape_until_scroll(&mut ctx.text.font_system, false);
            buffers.push(buf);
        }

        let mut text_areas: Vec<glyphon::TextArea<'_>> = Vec::new();
        for (i, buffer) in buffers.iter().enumerate() {
            let y = padding + (i as f32 * self.line_height);
            text_areas.push(glyphon::TextArea {
                buffer,
                left: padding,
                top: y,
                scale: 1.0,
                bounds: glyphon::TextBounds {
                    left: 0,
                    top: 0,
                    right: ctx.width as i32,
                    bottom: ctx.height as i32,
                },
                default_color: normal_color,
                custom_glyphs: &[],
            });
        }

        if let Err(e) = ctx.text.prepare(
            &ctx.gpu.device,
            &ctx.gpu.queue,
            ctx.width,
            ctx.height,
            text_areas,
        ) {
            tracing::warn!("text prepare error: {e}");
        }

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("kekkai_text"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: ctx.surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            if let Err(e) = ctx.text.render(&mut pass) {
                tracing::warn!("text render error: {e}");
            }
        }

        ctx.gpu.queue.submit(std::iter::once(encoder.finish()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::ConnectionInfo;

    #[test]
    fn state_default_mode_is_status() {
        let state = KekkaiState::new();
        assert_eq!(state.mode, ViewMode::Status);
    }

    #[test]
    fn set_servers_updates_list() {
        let mut state = KekkaiState::new();
        state.set_servers(vec![
            Server {
                id: 1,
                name: "US #100".into(),
                hostname: "us100.nordvpn.com".into(),
                country: "United States".into(),
                city: "New York".into(),
                load: 25,
                technologies: vec![],
                ip: "10.0.0.1".into(),
            },
            Server {
                id: 2,
                name: "DE #50".into(),
                hostname: "de50.nordvpn.com".into(),
                country: "Germany".into(),
                city: "Frankfurt".into(),
                load: 15,
                technologies: vec![],
                ip: "10.0.0.2".into(),
            },
        ]);
        assert_eq!(state.server_list.len(), 2);
        // Sorted by load, so Germany (15) should be first
        assert_eq!(state.display_servers[0].hostname, "de50.nordvpn.com");
    }

    #[test]
    fn sort_mode_cycles() {
        assert_eq!(SortMode::Load.next(), SortMode::Country);
        assert_eq!(SortMode::Country.next(), SortMode::Name);
        assert_eq!(SortMode::Name.next(), SortMode::Load);
    }

    #[test]
    fn sort_mode_labels() {
        assert_eq!(SortMode::Load.label(), "Load");
        assert_eq!(SortMode::Country.label(), "Country");
        assert_eq!(SortMode::Name.label(), "Name");
    }

    #[test]
    fn move_down_and_up() {
        let mut state = KekkaiState::new();
        state.set_servers(vec![
            Server {
                id: 1, name: "A".into(), hostname: "a.nordvpn.com".into(),
                country: "A".into(), city: "A".into(), load: 10,
                technologies: vec![], ip: "1.1.1.1".into(),
            },
            Server {
                id: 2, name: "B".into(), hostname: "b.nordvpn.com".into(),
                country: "B".into(), city: "B".into(), load: 20,
                technologies: vec![], ip: "2.2.2.2".into(),
            },
        ]);
        assert_eq!(state.server_list.selected_index(), 0);
        state.move_down();
        assert_eq!(state.server_list.selected_index(), 1);
        state.move_up();
        assert_eq!(state.server_list.selected_index(), 0);
    }

    #[test]
    fn toggle_favorite() {
        let mut state = KekkaiState::new();
        state.set_servers(vec![Server {
            id: 1, name: "US".into(), hostname: "us100.nordvpn.com".into(),
            country: "US".into(), city: "NYC".into(), load: 10,
            technologies: vec![], ip: "1.1.1.1".into(),
        }]);
        assert!(state.favorites.is_empty());
        state.toggle_favorite();
        assert_eq!(state.favorites.len(), 1);
        assert_eq!(state.favorites[0], "us100.nordvpn.com");
        state.toggle_favorite();
        assert!(state.favorites.is_empty());
    }

    #[test]
    fn enter_and_exit_search() {
        let mut state = KekkaiState::new();
        state.mode = ViewMode::ServerList;
        state.enter_search();
        assert_eq!(state.mode, ViewMode::Search);
        state.go_back();
        assert_eq!(state.mode, ViewMode::ServerList);
    }

    #[test]
    fn search_filters_servers() {
        let mut state = KekkaiState::new();
        state.set_servers(vec![
            Server {
                id: 1, name: "US #100".into(), hostname: "us100.nordvpn.com".into(),
                country: "United States".into(), city: "New York".into(), load: 25,
                technologies: vec![], ip: "10.0.0.1".into(),
            },
            Server {
                id: 2, name: "DE #50".into(), hostname: "de50.nordvpn.com".into(),
                country: "Germany".into(), city: "Frankfurt".into(), load: 15,
                technologies: vec![], ip: "10.0.0.2".into(),
            },
        ]);
        state.enter_search();
        state.search_input.insert_char('g');
        state.search_input.insert_char('e');
        state.search_input.insert_char('r');
        state.apply_search();
        assert_eq!(state.display_servers.len(), 1);
        assert_eq!(state.display_servers[0].country, "Germany");
    }

    #[test]
    fn go_back_from_list_to_status() {
        let mut state = KekkaiState::new();
        state.mode = ViewMode::ServerList;
        state.go_back();
        assert_eq!(state.mode, ViewMode::Status);
    }

    #[test]
    fn set_status_message() {
        let mut state = KekkaiState::new();
        state.set_status("Connected to us100");
        assert_eq!(state.status_message.as_deref(), Some("Connected to us100"));
    }

    #[test]
    fn load_bar_ranges() {
        assert_eq!(load_bar(5), "[====      ]");
        assert_eq!(load_bar(25), "[=====     ]");
        assert_eq!(load_bar(50), "[======    ]");
        assert_eq!(load_bar(70), "[========  ]");
        assert_eq!(load_bar(90), "[==========]");
    }

    #[test]
    fn collect_lines_status_disconnected() {
        let state = KekkaiState::new();
        let lines = collect_lines(&state);
        assert!(!lines.is_empty());
        assert!(lines.iter().any(|(l, _, _)| l.contains("Disconnected")));
    }

    #[test]
    fn collect_lines_status_connected() {
        let mut state = KekkaiState::new();
        state.connection = ConnectionState::Connected(ConnectionInfo {
            server: "us100.nordvpn.com".into(),
            protocol: "NordLynx".into(),
            ip: "10.0.0.1".into(),
            country: "United States".into(),
            city: "New York".into(),
            connected_at: None,
        });
        let lines = collect_lines(&state);
        assert!(lines.iter().any(|(l, _, _)| l.contains("us100")));
    }

    #[test]
    fn renderer_creates_with_defaults() {
        let appearance = crate::config::AppearanceConfig::default();
        let renderer = KekkaiRenderer::new(&appearance);
        assert_eq!(renderer.state.mode, ViewMode::Status);
    }

    #[test]
    fn server_grouping() {
        let servers = vec![
            Server {
                id: 1, name: "US #100".into(), hostname: "us100.nordvpn.com".into(),
                country: "United States".into(), city: "New York".into(), load: 25,
                technologies: vec![], ip: "10.0.0.1".into(),
            },
            Server {
                id: 2, name: "US #200".into(), hostname: "us200.nordvpn.com".into(),
                country: "United States".into(), city: "Los Angeles".into(), load: 40,
                technologies: vec![], ip: "10.0.0.2".into(),
            },
            Server {
                id: 3, name: "US #300".into(), hostname: "us300.nordvpn.com".into(),
                country: "United States".into(), city: "New York".into(), load: 30,
                technologies: vec![], ip: "10.0.0.3".into(),
            },
            Server {
                id: 4, name: "DE #50".into(), hostname: "de50.nordvpn.com".into(),
                country: "Germany".into(), city: "Frankfurt".into(), load: 15,
                technologies: vec![], ip: "10.0.0.4".into(),
            },
        ];
        let groups = build_server_groups(&servers, &[]);
        assert_eq!(groups.len(), 2); // 2 countries
        // Alphabetically: Germany, United States
        assert_eq!(groups[0].country, "Germany");
        assert_eq!(groups[1].country, "United States");
        assert_eq!(groups[1].cities.len(), 2); // 2 cities in US
    }

    #[test]
    fn server_grouping_favorites_first() {
        let servers = vec![
            Server {
                id: 1, name: "US #100".into(), hostname: "us100.nordvpn.com".into(),
                country: "United States".into(), city: "New York".into(), load: 25,
                technologies: vec![], ip: "10.0.0.1".into(),
            },
            Server {
                id: 2, name: "DE #50".into(), hostname: "de50.nordvpn.com".into(),
                country: "Germany".into(), city: "Frankfurt".into(), load: 15,
                technologies: vec![], ip: "10.0.0.2".into(),
            },
        ];
        let favorites = vec!["us100.nordvpn.com".to_string()];
        let groups = build_server_groups(&servers, &favorites);
        // US should come first because it has a favorite
        assert_eq!(groups[0].country, "United States");
    }

    #[test]
    fn city_group_avg_load() {
        let servers = vec![
            Server {
                id: 1, name: "US #100".into(), hostname: "us100.nordvpn.com".into(),
                country: "United States".into(), city: "New York".into(), load: 20,
                technologies: vec![], ip: "10.0.0.1".into(),
            },
            Server {
                id: 2, name: "US #200".into(), hostname: "us200.nordvpn.com".into(),
                country: "United States".into(), city: "New York".into(), load: 40,
                technologies: vec![], ip: "10.0.0.2".into(),
            },
        ];
        let groups = build_server_groups(&servers, &[]);
        assert_eq!(groups[0].cities[0].avg_load, 30);
    }

    #[test]
    fn cycle_protocol() {
        let mut state = KekkaiState::new();
        assert_eq!(state.protocol, "NordLynx");
        state.cycle_protocol();
        assert_eq!(state.protocol, "OpenVPN UDP");
        state.cycle_protocol();
        assert_eq!(state.protocol, "OpenVPN TCP");
        state.cycle_protocol();
        assert_eq!(state.protocol, "NordLynx");
    }

    #[test]
    fn favorites_only_filter() {
        let mut state = KekkaiState::new();
        state.set_servers(vec![
            Server {
                id: 1, name: "US #100".into(), hostname: "us100.nordvpn.com".into(),
                country: "United States".into(), city: "New York".into(), load: 25,
                technologies: vec![], ip: "10.0.0.1".into(),
            },
            Server {
                id: 2, name: "DE #50".into(), hostname: "de50.nordvpn.com".into(),
                country: "Germany".into(), city: "Frankfurt".into(), load: 15,
                technologies: vec![], ip: "10.0.0.2".into(),
            },
        ]);
        assert_eq!(state.display_servers.len(), 2);

        state.favorites.push("us100.nordvpn.com".into());
        state.favorites_only = true;
        state.refresh_display();
        assert_eq!(state.display_servers.len(), 1);
        assert_eq!(state.display_servers[0].hostname, "us100.nordvpn.com");
    }

    #[test]
    fn recommended_server_prefers_favorite() {
        let mut state = KekkaiState::new();
        state.set_servers(vec![
            Server {
                id: 1, name: "US #100".into(), hostname: "us100.nordvpn.com".into(),
                country: "United States".into(), city: "New York".into(), load: 30,
                technologies: vec![], ip: "10.0.0.1".into(),
            },
            Server {
                id: 2, name: "DE #50".into(), hostname: "de50.nordvpn.com".into(),
                country: "Germany".into(), city: "Frankfurt".into(), load: 15,
                technologies: vec![], ip: "10.0.0.2".into(),
            },
        ]);
        // Without favorites, should pick lowest load (Germany)
        let best = state.recommended_server().unwrap();
        assert_eq!(best.hostname, "de50.nordvpn.com");

        // With favorite on US, should prefer the favorite (load < 70)
        state.favorites.push("us100.nordvpn.com".into());
        let best = state.recommended_server().unwrap();
        assert_eq!(best.hostname, "us100.nordvpn.com");
    }

    #[test]
    fn go_back_from_map_to_status() {
        let mut state = KekkaiState::new();
        state.mode = ViewMode::ServerMap;
        state.go_back();
        assert_eq!(state.mode, ViewMode::Status);
    }

    #[test]
    fn collect_lines_server_map() {
        let mut state = KekkaiState::new();
        state.mode = ViewMode::ServerMap;
        state.set_servers(vec![
            Server {
                id: 1, name: "US #100".into(), hostname: "us100.nordvpn.com".into(),
                country: "United States".into(), city: "New York".into(), load: 25,
                technologies: vec![], ip: "10.0.0.1".into(),
            },
        ]);
        let lines = collect_lines(&state);
        assert!(lines.iter().any(|(l, _, _)| l.contains("Server Map")));
    }

    #[test]
    fn map_list_populated_on_set_servers() {
        let mut state = KekkaiState::new();
        state.set_servers(vec![
            Server {
                id: 1, name: "US #100".into(), hostname: "us100.nordvpn.com".into(),
                country: "United States".into(), city: "New York".into(), load: 25,
                technologies: vec![], ip: "10.0.0.1".into(),
            },
            Server {
                id: 2, name: "DE #50".into(), hostname: "de50.nordvpn.com".into(),
                country: "Germany".into(), city: "Frankfurt".into(), load: 15,
                technologies: vec![], ip: "10.0.0.2".into(),
            },
        ]);
        // Map list should have entries for countries and cities
        assert!(state.map_list.len() > 0);
    }

    #[test]
    fn protocol_shown_in_status() {
        let state = KekkaiState::new();
        let lines = collect_lines(&state);
        assert!(lines.iter().any(|(l, _, _)| l.contains("NordLynx")));
    }
}

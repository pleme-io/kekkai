//! GPU rendering module for server map and connection status.
//!
//! Uses madori (app framework) + garasu (GPU primitives) + egaku (widgets).
//!
//! # Planned rendering targets
//!
//! - **Server list** — scrollable list of NordVPN servers with load indicators
//!   (color-coded bars: green < 30%, yellow < 60%, red >= 60%).
//!
//! - **Connection status** — prominent display showing current state
//!   (connected/disconnected), server hostname, protocol, IP, and uptime.
//!
//! - **Country map** — GPU-rendered world map with server locations plotted
//!   as points, sized by server count per city, colored by average load.
//!   Selected country/city highlighted. Click-to-connect interaction.
//!
//! # Architecture
//!
//! ```text
//! madori::App
//!   ├── garasu::GpuContext     (wgpu device, surface, queue)
//!   ├── egaku::ServerList      (scrollable server table widget)
//!   ├── egaku::StatusBar       (connection state + quick actions)
//!   └── garasu::MapRenderer    (world map with server markers)
//! ```
//!
//! The madori app framework handles the event loop (winit), input routing,
//! and frame scheduling. garasu provides the low-level GPU pipeline (shaders,
//! buffers, render passes). egaku provides higher-level widgets (text, lists,
//! buttons) built on garasu primitives.
//!
//! # Implementation plan
//!
//! 1. Create `madori::App` with garasu GPU context
//! 2. Implement `egaku::StatusBar` showing `ConnectionState`
//! 3. Implement `egaku::ServerList` with filtering and sorting
//! 4. Add world map renderer with server markers
//! 5. Wire up click-to-connect and keyboard shortcuts

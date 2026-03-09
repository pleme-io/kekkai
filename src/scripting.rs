//! Rhai scripting integration for Kekkai.
//!
//! Loads user scripts from `~/.config/kekkai/scripts/*.rhai` and exposes
//! app-specific functions for VPN operations.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use soushi::ScriptEngine;

/// Script hook events that can trigger user scripts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptEvent {
    /// Successfully connected to a VPN server.
    Connected { server: String, country: String },
    /// Disconnected from VPN.
    Disconnected,
    /// Server list was refreshed.
    ServersRefreshed { count: usize },
    /// Connection attempt failed.
    ConnectionFailed { error: String },
}

/// Manages the Rhai scripting engine with kekkai-specific functions.
pub struct KekkaiScripting {
    engine: ScriptEngine,
    /// Compiled event hook scripts (ASTs keyed by event name).
    hooks: std::collections::HashMap<String, soushi::rhai::AST>,
}

impl KekkaiScripting {
    /// Create a new scripting engine with kekkai VPN functions registered.
    ///
    /// Registers: `kekkai.connect(server)`, `kekkai.disconnect()`,
    /// `kekkai.status()`, `kekkai.list_servers(country)`.
    ///
    /// The `action_tx` channel is used to send actions back to the main event loop.
    #[must_use]
    pub fn new(action_tx: Arc<Mutex<Vec<ScriptAction>>>) -> Self {
        let mut engine = ScriptEngine::new();
        engine.register_builtin_log();
        engine.register_builtin_env();
        engine.register_builtin_string();

        // kekkai.connect(server)
        let tx = action_tx.clone();
        engine.register_fn("kekkai_connect", move |server: &str| {
            if let Ok(mut actions) = tx.lock() {
                actions.push(ScriptAction::Connect(server.to_string()));
            }
        });

        // kekkai.disconnect()
        let tx = action_tx.clone();
        engine.register_fn("kekkai_disconnect", move || {
            if let Ok(mut actions) = tx.lock() {
                actions.push(ScriptAction::Disconnect);
            }
        });

        // kekkai.status()
        let tx = action_tx.clone();
        engine.register_fn("kekkai_status", move || -> String {
            if let Ok(mut actions) = tx.lock() {
                actions.push(ScriptAction::Status);
            }
            String::new()
        });

        // kekkai.list_servers(country)
        let tx = action_tx;
        engine.register_fn("kekkai_list_servers", move |country: &str| -> String {
            if let Ok(mut actions) = tx.lock() {
                actions.push(ScriptAction::ListServers(country.to_string()));
            }
            String::new()
        });

        Self {
            engine,
            hooks: std::collections::HashMap::new(),
        }
    }

    /// Load all scripts from the scripts directory.
    ///
    /// Looks in `~/.config/kekkai/scripts/` by default.
    pub fn load_scripts(&mut self) -> Result<Vec<String>, soushi::SoushiError> {
        let scripts_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("kekkai")
            .join("scripts");

        if !scripts_dir.is_dir() {
            tracing::debug!(path = %scripts_dir.display(), "scripts directory not found, skipping");
            return Ok(Vec::new());
        }

        self.engine.load_scripts_dir(&scripts_dir)
    }

    /// Register an event hook script.
    pub fn register_hook(&mut self, event_name: &str, script: &str) -> Result<(), soushi::SoushiError> {
        let ast = self.engine.compile(script)?;
        self.hooks.insert(event_name.to_string(), ast);
        Ok(())
    }

    /// Fire an event, running any registered hook scripts.
    pub fn fire_event(&self, event: &ScriptEvent) {
        let event_name = match event {
            ScriptEvent::Connected { .. } => "connected",
            ScriptEvent::Disconnected => "disconnected",
            ScriptEvent::ServersRefreshed { .. } => "servers_refreshed",
            ScriptEvent::ConnectionFailed { .. } => "connection_failed",
        };

        if let Some(ast) = self.hooks.get(event_name) {
            if let Err(e) = self.engine.eval_ast(ast) {
                tracing::error!(event = event_name, error = %e, "script hook failed");
            }
        }
    }

    /// Evaluate an ad-hoc script string.
    pub fn eval(&self, script: &str) -> Result<soushi::rhai::Dynamic, soushi::SoushiError> {
        self.engine.eval(script)
    }
}

/// Actions that scripts can request from the application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptAction {
    /// Connect to a named server.
    Connect(String),
    /// Disconnect from VPN.
    Disconnect,
    /// Request connection status.
    Status,
    /// List servers for a country.
    ListServers(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_engine() -> (KekkaiScripting, Arc<Mutex<Vec<ScriptAction>>>) {
        let actions = Arc::new(Mutex::new(Vec::new()));
        let engine = KekkaiScripting::new(actions.clone());
        (engine, actions)
    }

    #[test]
    fn connect_function_queues_action() {
        let (engine, actions) = make_engine();
        engine.eval(r#"kekkai_connect("us1234")"#).unwrap();
        let actions = actions.lock().unwrap();
        assert_eq!(actions[0], ScriptAction::Connect("us1234".to_string()));
    }

    #[test]
    fn disconnect_function_queues_action() {
        let (engine, actions) = make_engine();
        engine.eval("kekkai_disconnect()").unwrap();
        let actions = actions.lock().unwrap();
        assert_eq!(actions[0], ScriptAction::Disconnect);
    }

    #[test]
    fn status_function_queues_action() {
        let (engine, actions) = make_engine();
        engine.eval("kekkai_status()").unwrap();
        let actions = actions.lock().unwrap();
        assert_eq!(actions[0], ScriptAction::Status);
    }

    #[test]
    fn list_servers_function_queues_action() {
        let (engine, actions) = make_engine();
        engine.eval(r#"kekkai_list_servers("US")"#).unwrap();
        let actions = actions.lock().unwrap();
        assert_eq!(actions[0], ScriptAction::ListServers("US".to_string()));
    }

    #[test]
    fn fire_event_with_no_hook_is_noop() {
        let (engine, _actions) = make_engine();
        engine.fire_event(&ScriptEvent::Disconnected);
    }

    #[test]
    fn register_and_fire_hook() {
        let (mut engine, actions) = make_engine();
        engine
            .register_hook("disconnected", r#"kekkai_connect("us1234")"#)
            .unwrap();
        engine.fire_event(&ScriptEvent::Disconnected);
        let actions = actions.lock().unwrap();
        assert_eq!(actions[0], ScriptAction::Connect("us1234".to_string()));
    }

    #[test]
    fn load_scripts_missing_dir_returns_empty() {
        let (mut engine, _actions) = make_engine();
        let result = engine.load_scripts();
        assert!(result.is_ok());
    }

    #[test]
    fn eval_arbitrary_script() {
        let (engine, _actions) = make_engine();
        let result = engine.eval("40 + 2").unwrap();
        assert_eq!(result.as_int().unwrap(), 42);
    }
}

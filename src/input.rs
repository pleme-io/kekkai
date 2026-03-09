//! Input handling -- vim-style keyboard navigation.
//!
//! Dispatches madori `AppEvent::Key` events to the appropriate action
//! based on the current view mode (Normal, ServerList, Map).
//! Uses awase types for hotkey representation and parsing.

use crate::render::ViewMode;
use madori::event::KeyCode;

/// Actions that can be triggered by keyboard input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Navigate items down.
    Down,
    /// Navigate items up.
    Up,
    /// Quick connect to preferred/recommended server.
    QuickConnect,
    /// Disconnect from VPN.
    Disconnect,
    /// Show status panel.
    ShowStatus,
    /// Switch to map view.
    SwitchToMap,
    /// Switch to server list view.
    SwitchToList,
    /// Toggle favorites.
    ToggleFavorite,
    /// Toggle favorites-only filter.
    ToggleFavoritesOnly,
    /// Enter search mode.
    EnterSearch,
    /// Connect to selected server.
    ConnectSelected,
    /// Cycle sort mode (in list view).
    CycleSort,
    /// Cycle protocol (NordLynx/OpenVPN).
    CycleProtocol,
    /// Go back / exit current mode.
    Back,
    /// Quit the application.
    Quit,
    /// Insert character into search input.
    SearchInput(char),
    /// Delete character from search input (backspace).
    SearchBackspace,
    /// Submit search query.
    SearchSubmit,
    /// Switch to next view (Tab).
    NextView,
    /// No action.
    None,
}

/// Convert a madori key event to an awase `Hotkey` (when possible).
///
/// Provides a bridge between madori's input events and awase's hotkey
/// system, enabling user-configurable keybindings via awase's
/// `Hotkey::parse()` format (e.g., `"cmd+q"`, `"ctrl+d"`).
#[must_use]
pub fn to_awase_hotkey(
    key: &KeyCode,
    modifiers: &madori::event::Modifiers,
) -> Option<awase::Hotkey> {
    let awase_key = madori_key_to_awase(key)?;
    let mut mods = awase::Modifiers::NONE;
    if modifiers.shift {
        mods |= awase::Modifiers::SHIFT;
    }
    if modifiers.ctrl {
        mods |= awase::Modifiers::CTRL;
    }
    if modifiers.alt {
        mods |= awase::Modifiers::ALT;
    }
    if modifiers.meta {
        mods |= awase::Modifiers::CMD;
    }
    Some(awase::Hotkey::new(mods, awase_key))
}

/// Map a madori `KeyCode` to an awase `Key`.
fn madori_key_to_awase(key: &KeyCode) -> Option<awase::Key> {
    match key {
        KeyCode::Char(c) => match c.to_ascii_lowercase() {
            'a' => Some(awase::Key::A), 'b' => Some(awase::Key::B),
            'c' => Some(awase::Key::C), 'd' => Some(awase::Key::D),
            'e' => Some(awase::Key::E), 'f' => Some(awase::Key::F),
            'g' => Some(awase::Key::G), 'h' => Some(awase::Key::H),
            'i' => Some(awase::Key::I), 'j' => Some(awase::Key::J),
            'k' => Some(awase::Key::K), 'l' => Some(awase::Key::L),
            'm' => Some(awase::Key::M), 'n' => Some(awase::Key::N),
            'o' => Some(awase::Key::O), 'p' => Some(awase::Key::P),
            'q' => Some(awase::Key::Q), 'r' => Some(awase::Key::R),
            's' => Some(awase::Key::S), 't' => Some(awase::Key::T),
            'u' => Some(awase::Key::U), 'v' => Some(awase::Key::V),
            'w' => Some(awase::Key::W), 'x' => Some(awase::Key::X),
            'y' => Some(awase::Key::Y), 'z' => Some(awase::Key::Z),
            '0' => Some(awase::Key::Num0), '1' => Some(awase::Key::Num1),
            '2' => Some(awase::Key::Num2), '3' => Some(awase::Key::Num3),
            '4' => Some(awase::Key::Num4), '5' => Some(awase::Key::Num5),
            '6' => Some(awase::Key::Num6), '7' => Some(awase::Key::Num7),
            '8' => Some(awase::Key::Num8), '9' => Some(awase::Key::Num9),
            '/' => Some(awase::Key::Slash),
            '+' | '=' => Some(awase::Key::Equal),
            '-' => Some(awase::Key::Minus),
            ',' => Some(awase::Key::Comma),
            '.' => Some(awase::Key::Period),
            _ => None,
        },
        KeyCode::Space => Some(awase::Key::Space),
        KeyCode::Enter => Some(awase::Key::Return),
        KeyCode::Escape => Some(awase::Key::Escape),
        KeyCode::Tab => Some(awase::Key::Tab),
        KeyCode::Backspace => Some(awase::Key::Backspace),
        KeyCode::Delete => Some(awase::Key::Delete),
        KeyCode::Up => Some(awase::Key::Up),
        KeyCode::Down => Some(awase::Key::Down),
        KeyCode::Left => Some(awase::Key::Left),
        KeyCode::Right => Some(awase::Key::Right),
        KeyCode::Home => Some(awase::Key::Home),
        KeyCode::End => Some(awase::Key::End),
        KeyCode::PageUp => Some(awase::Key::PageUp),
        KeyCode::PageDown => Some(awase::Key::PageDown),
        _ => None,
    }
}

/// Check if a key event matches an awase hotkey string.
///
/// Enables config-driven keybinding lookups.
#[must_use]
pub fn matches_hotkey(
    key: &KeyCode,
    modifiers: &madori::event::Modifiers,
    hotkey_str: &str,
) -> bool {
    let Some(event_hk) = to_awase_hotkey(key, modifiers) else {
        return false;
    };
    awase::Hotkey::parse(hotkey_str)
        .map(|parsed| parsed == event_hk)
        .unwrap_or(false)
}

/// Map a key event to an action based on the current view mode.
#[must_use]
pub fn map_key(
    key: &KeyCode,
    pressed: bool,
    _modifiers: &madori::event::Modifiers,
    text: &Option<String>,
    mode: &ViewMode,
) -> Action {
    if !pressed {
        return Action::None;
    }

    match mode {
        ViewMode::Status => map_status(key),
        ViewMode::ServerList => map_list(key),
        ViewMode::ServerMap => map_map(key),
        ViewMode::Search => map_search(key, text),
    }
}

fn map_status(key: &KeyCode) -> Action {
    match key {
        KeyCode::Char('c') => Action::QuickConnect,
        KeyCode::Char('d') => Action::Disconnect,
        KeyCode::Char('l') => Action::SwitchToList,
        KeyCode::Char('m') => Action::SwitchToMap,
        KeyCode::Char('f') => Action::ToggleFavorite,
        KeyCode::Char('p') => Action::CycleProtocol,
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Tab => Action::NextView,
        _ => Action::None,
    }
}

fn map_list(key: &KeyCode) -> Action {
    match key {
        KeyCode::Char('j') | KeyCode::Down => Action::Down,
        KeyCode::Char('k') | KeyCode::Up => Action::Up,
        KeyCode::Enter => Action::ConnectSelected,
        KeyCode::Char('f') => Action::ToggleFavorite,
        KeyCode::Char('s') => Action::CycleSort,
        KeyCode::Char('p') => Action::CycleProtocol,
        KeyCode::Char('/') => Action::EnterSearch,
        KeyCode::Char('d') => Action::Disconnect,
        KeyCode::Escape => Action::Back,
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Tab => Action::NextView,
        _ => Action::None,
    }
}

fn map_map(key: &KeyCode) -> Action {
    match key {
        KeyCode::Char('j') | KeyCode::Down => Action::Down,
        KeyCode::Char('k') | KeyCode::Up => Action::Up,
        KeyCode::Char('f') => Action::ToggleFavoritesOnly,
        KeyCode::Char('/') => Action::EnterSearch,
        KeyCode::Escape => Action::Back,
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Tab => Action::NextView,
        _ => Action::None,
    }
}

fn map_search(key: &KeyCode, text: &Option<String>) -> Action {
    match key {
        KeyCode::Escape => Action::Back,
        KeyCode::Enter => Action::SearchSubmit,
        KeyCode::Backspace => Action::SearchBackspace,
        _ => {
            if let Some(t) = text {
                if let Some(c) = t.chars().next() {
                    return Action::SearchInput(c);
                }
            }
            if let KeyCode::Char(c) = key {
                Action::SearchInput(*c)
            } else {
                Action::None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_mods() -> madori::event::Modifiers {
        madori::event::Modifiers::default()
    }

    #[test]
    fn status_c_quick_connects() {
        let action = map_key(&KeyCode::Char('c'), true, &no_mods(), &None, &ViewMode::Status);
        assert_eq!(action, Action::QuickConnect);
    }

    #[test]
    fn status_d_disconnects() {
        let action = map_key(&KeyCode::Char('d'), true, &no_mods(), &None, &ViewMode::Status);
        assert_eq!(action, Action::Disconnect);
    }

    #[test]
    fn status_l_switches_to_list() {
        let action = map_key(&KeyCode::Char('l'), true, &no_mods(), &None, &ViewMode::Status);
        assert_eq!(action, Action::SwitchToList);
    }

    #[test]
    fn status_q_quits() {
        let action = map_key(&KeyCode::Char('q'), true, &no_mods(), &None, &ViewMode::Status);
        assert_eq!(action, Action::Quit);
    }

    #[test]
    fn list_j_moves_down() {
        let action = map_key(&KeyCode::Char('j'), true, &no_mods(), &None, &ViewMode::ServerList);
        assert_eq!(action, Action::Down);
    }

    #[test]
    fn list_k_moves_up() {
        let action = map_key(&KeyCode::Char('k'), true, &no_mods(), &None, &ViewMode::ServerList);
        assert_eq!(action, Action::Up);
    }

    #[test]
    fn list_enter_connects() {
        let action = map_key(&KeyCode::Enter, true, &no_mods(), &None, &ViewMode::ServerList);
        assert_eq!(action, Action::ConnectSelected);
    }

    #[test]
    fn list_slash_enters_search() {
        let action = map_key(&KeyCode::Char('/'), true, &no_mods(), &None, &ViewMode::ServerList);
        assert_eq!(action, Action::EnterSearch);
    }

    #[test]
    fn list_f_toggles_favorite() {
        let action = map_key(&KeyCode::Char('f'), true, &no_mods(), &None, &ViewMode::ServerList);
        assert_eq!(action, Action::ToggleFavorite);
    }

    #[test]
    fn list_s_cycles_sort() {
        let action = map_key(&KeyCode::Char('s'), true, &no_mods(), &None, &ViewMode::ServerList);
        assert_eq!(action, Action::CycleSort);
    }

    #[test]
    fn list_escape_goes_back() {
        let action = map_key(&KeyCode::Escape, true, &no_mods(), &None, &ViewMode::ServerList);
        assert_eq!(action, Action::Back);
    }

    #[test]
    fn search_escape_goes_back() {
        let action = map_key(&KeyCode::Escape, true, &no_mods(), &None, &ViewMode::Search);
        assert_eq!(action, Action::Back);
    }

    #[test]
    fn search_text_input() {
        let action = map_key(
            &KeyCode::Char('a'),
            true,
            &no_mods(),
            &Some("a".into()),
            &ViewMode::Search,
        );
        assert_eq!(action, Action::SearchInput('a'));
    }

    #[test]
    fn search_backspace() {
        let action = map_key(&KeyCode::Backspace, true, &no_mods(), &None, &ViewMode::Search);
        assert_eq!(action, Action::SearchBackspace);
    }

    #[test]
    fn search_enter_submits() {
        let action = map_key(&KeyCode::Enter, true, &no_mods(), &None, &ViewMode::Search);
        assert_eq!(action, Action::SearchSubmit);
    }

    #[test]
    fn key_release_is_noop() {
        let action = map_key(&KeyCode::Char('j'), false, &no_mods(), &None, &ViewMode::ServerList);
        assert_eq!(action, Action::None);
    }

    #[test]
    fn tab_switches_view() {
        let action = map_key(&KeyCode::Tab, true, &no_mods(), &None, &ViewMode::Status);
        assert_eq!(action, Action::NextView);
    }

    #[test]
    fn status_p_cycles_protocol() {
        let action = map_key(&KeyCode::Char('p'), true, &no_mods(), &None, &ViewMode::Status);
        assert_eq!(action, Action::CycleProtocol);
    }

    #[test]
    fn list_p_cycles_protocol() {
        let action = map_key(&KeyCode::Char('p'), true, &no_mods(), &None, &ViewMode::ServerList);
        assert_eq!(action, Action::CycleProtocol);
    }

    #[test]
    fn map_j_moves_down() {
        let action = map_key(&KeyCode::Char('j'), true, &no_mods(), &None, &ViewMode::ServerMap);
        assert_eq!(action, Action::Down);
    }

    #[test]
    fn map_k_moves_up() {
        let action = map_key(&KeyCode::Char('k'), true, &no_mods(), &None, &ViewMode::ServerMap);
        assert_eq!(action, Action::Up);
    }

    #[test]
    fn map_f_toggles_favorites_only() {
        let action = map_key(&KeyCode::Char('f'), true, &no_mods(), &None, &ViewMode::ServerMap);
        assert_eq!(action, Action::ToggleFavoritesOnly);
    }

    #[test]
    fn map_escape_goes_back() {
        let action = map_key(&KeyCode::Escape, true, &no_mods(), &None, &ViewMode::ServerMap);
        assert_eq!(action, Action::Back);
    }

    #[test]
    fn map_q_quits() {
        let action = map_key(&KeyCode::Char('q'), true, &no_mods(), &None, &ViewMode::ServerMap);
        assert_eq!(action, Action::Quit);
    }

    #[test]
    fn map_tab_switches_view() {
        let action = map_key(&KeyCode::Tab, true, &no_mods(), &None, &ViewMode::ServerMap);
        assert_eq!(action, Action::NextView);
    }

    #[test]
    fn map_slash_enters_search() {
        let action = map_key(&KeyCode::Char('/'), true, &no_mods(), &None, &ViewMode::ServerMap);
        assert_eq!(action, Action::EnterSearch);
    }

    #[test]
    fn status_m_switches_to_map() {
        let action = map_key(&KeyCode::Char('m'), true, &no_mods(), &None, &ViewMode::Status);
        assert_eq!(action, Action::SwitchToMap);
    }

    // -- awase integration tests --

    #[test]
    fn to_awase_hotkey_converts_char() {
        let hk = to_awase_hotkey(&KeyCode::Char('j'), &no_mods()).unwrap();
        assert_eq!(hk.key, awase::Key::J);
        assert!(hk.modifiers.is_empty());
    }

    #[test]
    fn to_awase_hotkey_with_shift() {
        let mods = madori::event::Modifiers {
            shift: true,
            ..Default::default()
        };
        let hk = to_awase_hotkey(&KeyCode::Char('J'), &mods).unwrap();
        assert_eq!(hk.key, awase::Key::J);
        assert!(hk.modifiers.contains(awase::Modifiers::SHIFT));
    }

    #[test]
    fn matches_hotkey_basic() {
        assert!(matches_hotkey(&KeyCode::Char('q'), &no_mods(), "q"));
        assert!(!matches_hotkey(&KeyCode::Char('q'), &no_mods(), "w"));
    }

    #[test]
    fn matches_hotkey_with_modifier() {
        let mods = madori::event::Modifiers {
            ctrl: true,
            ..Default::default()
        };
        assert!(matches_hotkey(&KeyCode::Char('d'), &mods, "ctrl+d"));
        assert!(!matches_hotkey(&KeyCode::Char('d'), &mods, "d"));
    }

    #[test]
    fn awase_hotkey_parse_roundtrip() {
        let hk = awase::Hotkey::parse("cmd+q").unwrap();
        assert_eq!(hk.modifiers, awase::Modifiers::CMD);
        assert_eq!(hk.key, awase::Key::Q);
    }
}

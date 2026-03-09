//! Input handling — vim-style keyboard navigation.
//!
//! Dispatches madori `AppEvent::Key` events to the appropriate action
//! based on the current view mode (Normal, ServerList, Map).

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
}

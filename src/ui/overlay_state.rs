//! Pure lifecycle rules: tests never create a window or register a hotkey.
#[derive(Debug, Default)]
pub struct Lifecycle {
    epoch: u64,
    visible: bool,
    focused: bool,
}
impl Lifecycle {
    pub fn show(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
        self.visible = true;
        self.focused = false;
    }
    pub fn focus(&mut self) {
        if self.visible {
            self.focused = true;
        }
    }
    pub fn hide(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
        self.visible = false;
        self.focused = false;
    }
    pub fn deactivation(&self) -> Option<u64> {
        (self.visible && self.focused).then_some(self.epoch)
    }
    pub fn dismiss(&self, epoch: u64, own_foreground: bool) -> bool {
        self.visible && self.focused && self.epoch == epoch && !own_foreground
    }
    pub fn current(&self) -> Option<u64> {
        self.visible.then_some(self.epoch)
    }
}

pub fn restore_allowed(
    palette_is_foreground: bool,
    previous_exists: bool,
    same_process: bool,
) -> bool {
    palette_is_foreground && previous_exists && same_process
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn initial_focus_events_internal_focus_and_stale_dismissals_are_safe() {
        let mut state = Lifecycle::default();
        assert_eq!(state.deactivation(), None);
        state.show();
        assert_eq!(state.deactivation(), None);
        state.focus();
        let first = state.deactivation().unwrap();
        assert!(!state.dismiss(first, true));
        assert!(state.dismiss(first, false));
        state.hide();
        assert!(!state.dismiss(first, false));
        state.show();
        state.focus();
        assert!(!state.dismiss(first, false));
        assert!(state.dismiss(state.deactivation().unwrap(), false));
    }
    #[test]
    fn repeated_show_hide_has_no_pending_focus_or_dismissal_after_hiding() {
        let mut state = Lifecycle::default();
        for _ in 0..100 {
            state.show();
            state.focus();
            state.hide();
            assert_eq!(state.deactivation(), None);
        }
    }
    #[test]
    fn restoring_focus_never_steals_from_an_outside_click_or_reused_handle() {
        assert!(restore_allowed(true, true, true));
        assert!(!restore_allowed(false, true, true));
        assert!(!restore_allowed(true, false, true));
        assert!(!restore_allowed(true, true, false));
    }
}

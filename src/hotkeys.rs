//! Registration policy independent of Windows; production calls stay on one owning thread.
use anyhow::{Result, bail};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Alt,
    ControlAlt,
    ControlShift,
    AltShift,
}
impl Key {
    pub const ALL: [Self; 4] = [
        Self::Alt,
        Self::ControlAlt,
        Self::ControlShift,
        Self::AltShift,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Alt => "Alt+Space",
            Self::ControlAlt => "Ctrl+Alt+Space",
            Self::ControlShift => "Ctrl+Shift+Space",
            Self::AltShift => "Alt+Shift+Space",
        }
    }
    pub fn parse(s: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|k| k.label() == s)
            .ok_or_else(|| anyhow::anyhow!("Choose a supported Space shortcut"))
    }
    pub fn modifiers(self) -> u32 {
        match self {
            Self::Alt => 1,
            Self::ControlAlt => 3,
            Self::ControlShift => 6,
            Self::AltShift => 5,
        }
    }
}
pub trait Backend {
    fn register(&mut self, id: i32, key: Key) -> Result<()>;
    fn unregister(&mut self, id: i32) -> Result<()>;
}
pub struct Manager<B: Backend> {
    pub backend: B,
    pub active: Option<(i32, Key)>,
    pub status: String,
    pending_cleanup: Option<i32>,
}
impl<B: Backend> Manager<B> {
    pub fn start(backend: B, preferred: Key) -> Self {
        let mut manager = Self {
            backend,
            active: None,
            status: String::new(),
            pending_cleanup: None,
        };
        let mut errors = Vec::new();
        for key in
            std::iter::once(preferred).chain(Key::ALL.into_iter().filter(|k| *k != preferred))
        {
            match manager.backend.register(1, key) {
                Ok(()) => {
                    manager.active = Some((1, key));
                    break;
                }
                Err(e) => errors.push(format!("{}: {e}", key.label())),
            }
        }
        manager.status = match manager.active {
            Some((_, key)) if errors.is_empty() => format!("Shortcut: {}", key.label()),
            Some((_, key)) => format!(
                "Shortcut: {} (preferred unavailable: {})",
                key.label(),
                errors.join("; ")
            ),
            None => format!(
                "No shortcut available. Close competing apps, then restart; use --settings for recovery. {}",
                errors.join("; ")
            ),
        };
        manager
    }
    pub fn switch(&mut self, key: Key, mut save: impl FnMut(Key) -> Result<()>) -> Result<()> {
        if let Some(id) = self.pending_cleanup {
            self.backend.unregister(id)?;
            self.pending_cleanup = None;
        }
        if self.active.is_some_and(|(_, old)| old == key) {
            save(key)?;
            self.status = format!("Shortcut: {}", key.label());
            return Ok(());
        }
        let id = if self.active.is_some_and(|(id, _)| id == 1) {
            2
        } else {
            1
        };
        self.backend.register(id, key)?;
        self.pending_cleanup = Some(id);
        if let Err(e) = save(key) {
            let rollback = self.backend.unregister(id);
            if rollback.is_ok() {
                self.pending_cleanup = None;
            }
            return Err(e.context(format!("New shortcut cancelled; cleanup: {rollback:?}")));
        }
        if let Some((old_id, old_key)) = self.active
            && let Err(e) = self.backend.unregister(old_id)
        {
            let persisted = save(old_key);
            let cleanup = self.backend.unregister(id);
            if cleanup.is_ok() {
                self.pending_cleanup = None;
            }
            bail!(
                "Old shortcut could not be retired: {e}; preference rollback: {persisted:?}; new registration cleanup: {cleanup:?}"
            );
        }
        self.active = Some((id, key));
        self.pending_cleanup = None;
        self.status = format!("Shortcut: {}", key.label());
        Ok(())
    }
}
impl<B: Backend> Drop for Manager<B> {
    fn drop(&mut self) {
        if let Some(id) = self.pending_cleanup {
            let _ = self.backend.unregister(id);
        }
        if let Some((id, _)) = self.active {
            let _ = self.backend.unregister(id);
        }
    }
}

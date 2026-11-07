//! Event-driven command queue. Register/unregister always run on the original owner.
use super::{Launcher, Settings};
use anyhow::Result;
use slint::ComponentHandle;
use spotlight_prototype::{
    hotkeys::{Backend, Key, Manager},
    preferences::{Change, Document},
};
use std::{
    sync::{Arc, Mutex, mpsc},
    thread,
};
use windows::Win32::{
    Foundation::{LPARAM, WPARAM},
    UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};
const CHANGE: u32 = WM_APP + 31;
struct Native;
impl Backend for Native {
    fn register(&mut self, id: i32, key: Key) -> Result<()> {
        unsafe {
            RegisterHotKey(
                None,
                id,
                HOT_KEY_MODIFIERS(key.modifiers()) | MOD_NOREPEAT,
                0x20,
            )?;
        }
        Ok(())
    }
    fn unregister(&mut self, id: i32) -> Result<()> {
        unsafe {
            UnregisterHotKey(None, id)?;
        }
        Ok(())
    }
}
#[derive(Clone)]
pub struct Client {
    thread: u32,
    sender: mpsc::Sender<Key>,
}
impl Client {
    pub fn change(&self, key: Key) -> Result<()> {
        self.sender.send(key)?;
        unsafe {
            PostThreadMessageW(self.thread, CHANGE, WPARAM(0), LPARAM(0))?;
        }
        Ok(())
    }
}
pub struct Hotkey {
    pub client: Client,
    join: Option<thread::JoinHandle<()>>,
}
impl Hotkey {
    pub fn start(
        preferred: Key,
        weak: slint::Weak<Launcher>,
        settings: slint::Weak<Settings>,
        document: Arc<Mutex<Option<Document>>>,
    ) -> Result<(Self, String)> {
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let (sender, receiver) = mpsc::channel();
        let join = thread::spawn(move || unsafe {
            let tid = windows::Win32::System::Threading::GetCurrentThreadId();
            let mut msg = MSG::default();
            let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);
            let mut manager = Manager::start(Native, preferred);
            let _ = ready_tx.send((tid, manager.status.clone()));
            while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
                if msg.message == CHANGE {
                    while let Ok(key) = receiver.try_recv() {
                        let result = manager.switch(key, |key| {
                            document
                                .lock()
                                .unwrap()
                                .as_mut()
                                .ok_or_else(|| {
                                    anyhow::anyhow!("Repair config.json and restart before saving")
                                })?
                                .update(Change::Hotkey(key))
                        });
                        let status = manager.status.clone();
                        let active = manager.active.map(|(_, key)| key);
                        let error = result
                            .err()
                            .map(|e| format!("Shortcut unchanged: {e:#}"))
                            .unwrap_or_default();
                        let weak = weak.clone();
                        let settings = settings.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = weak.upgrade() {
                                ui.set_shortcut(status.clone().into());
                            }
                            if let Some(s) = settings.upgrade() {
                                s.set_shortcut_status(status.into());
                                s.set_shortcut_busy(false);
                                s.set_message(error.into());
                                if let Some(active) = active {
                                    s.set_shortcut_index(
                                        Key::ALL.iter().position(|k| *k == active).unwrap() as i32,
                                    );
                                }
                            }
                        });
                    }
                } else if msg.message == WM_HOTKEY
                    && manager
                        .active
                        .is_some_and(|(id, _)| msg.wParam.0 == id as usize)
                {
                    spotlight_prototype::diagnostics::record("hotkey_received", 0);
                    let weak = weak.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = weak.upgrade() {
                            if ui.window().is_visible() {
                                let _ = super::window::hide(&ui);
                            } else {
                                let _ = super::window::show(&ui);
                            }
                        }
                    });
                }
            }
            // Manager's Drop unregisters the actual active ID on this thread.
        });
        let (thread, label) = ready_rx.recv()?;
        Ok((
            Self {
                client: Client { thread, sender },
                join: Some(join),
            },
            label,
        ))
    }
}
impl Drop for Hotkey {
    fn drop(&mut self) {
        if self.join.as_ref().is_some_and(|j| !j.is_finished()) {
            unsafe {
                let _ = PostThreadMessageW(self.client.thread, WM_QUIT, WPARAM(0), LPARAM(0));
            }
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

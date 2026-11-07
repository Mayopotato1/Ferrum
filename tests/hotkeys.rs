use spotlight_prototype::hotkeys::*;
use std::{cell::RefCell, rc::Rc};
#[derive(Default)]
struct Fake {
    entries: Vec<(i32, Key)>,
    blocked: Vec<Key>,
    log: Rc<RefCell<Vec<String>>>,
}
impl Backend for Fake {
    fn register(&mut self, id: i32, key: Key) -> anyhow::Result<()> {
        if self.blocked.contains(&key) {
            anyhow::bail!("In use");
        }
        self.entries.push((id, key));
        self.log.borrow_mut().push(format!("register {id}"));
        Ok(())
    }
    fn unregister(&mut self, id: i32) -> anyhow::Result<()> {
        self.entries.retain(|(i, _)| *i != id);
        self.log.borrow_mut().push(format!("unregister {id}"));
        Ok(())
    }
}
#[test]
fn successful_switch_registers_then_persists_then_retires_and_shutdown_cleans_up() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut m = Manager::start(
        Fake {
            log: log.clone(),
            ..Default::default()
        },
        Key::Alt,
    );
    m.switch(Key::ControlShift, |_| {
        log.borrow_mut().push("save".into());
        Ok(())
    })
    .unwrap();
    assert_eq!(m.active, Some((2, Key::ControlShift)));
    assert_eq!(m.backend.entries, [(2, Key::ControlShift)]);
    assert_eq!(
        *log.borrow(),
        ["register 1", "register 2", "save", "unregister 1"]
    );
    drop(m);
    assert_eq!(log.borrow().last().unwrap(), "unregister 2");
}
#[test]
fn conflict_and_save_failure_leave_old_shortcut_working() {
    let mut m = Manager::start(Fake::default(), Key::Alt);
    m.backend.blocked.push(Key::ControlAlt);
    assert!(
        m.switch(Key::ControlAlt, |_| panic!(
            "Must not save failed registration"
        ))
        .is_err()
    );
    assert_eq!(m.active, Some((1, Key::Alt)));
    assert!(
        m.switch(Key::AltShift, |_| anyhow::bail!("No write permission"))
            .is_err()
    );
    assert_eq!(m.backend.entries, [(1, Key::Alt)]);
}
#[test]
fn startup_fallback_is_reported_and_no_hotkey_can_be_recovered() {
    let mut m = Manager::start(
        Fake {
            blocked: vec![Key::Alt],
            ..Default::default()
        },
        Key::Alt,
    );
    assert_eq!(m.active, Some((1, Key::ControlAlt)));
    assert!(m.status.contains("preferred unavailable"));
    assert!(Key::parse("Ctrl+Q").is_err());
    assert!(Key::parse("Alt+F4").is_err());
    for k in Key::ALL {
        assert_eq!(Key::parse(k.label()).unwrap(), k);
    }
    let mut none = Manager::start(
        Fake {
            blocked: Key::ALL.to_vec(),
            ..Default::default()
        },
        Key::Alt,
    );
    assert!(none.active.is_none());
    assert!(none.status.contains("--settings"));
    none.backend.blocked.clear();
    none.switch(Key::Alt, |_| Ok(())).unwrap();
    assert!(none.active.is_some());
    m.switch(Key::ControlAlt, |_| Ok(())).unwrap();
}
#[test]
fn retiring_old_registration_failure_rolls_back_saved_key() {
    struct FailRetire(Fake);
    impl Backend for FailRetire {
        fn register(&mut self, id: i32, key: Key) -> anyhow::Result<()> {
            self.0.register(id, key)
        }
        fn unregister(&mut self, id: i32) -> anyhow::Result<()> {
            if id == 1 {
                anyhow::bail!("Retirement failed");
            }
            self.0.unregister(id)
        }
    }
    let mut m = Manager::start(FailRetire(Fake::default()), Key::Alt);
    let mut persisted = Key::Alt;
    assert!(
        m.switch(Key::ControlShift, |key| {
            persisted = key;
            Ok(())
        })
        .is_err()
    );
    assert_eq!(persisted, Key::Alt);
    assert_eq!(m.active, Some((1, Key::Alt)));
    assert_eq!(m.backend.0.entries, [(1, Key::Alt)]);
}

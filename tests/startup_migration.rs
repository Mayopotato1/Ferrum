use spotlight_prototype::{startup, startup_migration::*};
#[derive(Default)]
struct Memory {
    values: [Option<String>; 2],
    fail: bool,
    writes: usize,
}
fn index(slot: Slot) -> usize {
    if slot == Slot::Legacy { 0 } else { 1 }
}
impl Entries for Memory {
    fn read(&self, slot: Slot) -> anyhow::Result<Option<String>> {
        Ok(self.values[index(slot)].clone())
    }
    fn write(&mut self, slot: Slot, v: Option<&str>) -> anyhow::Result<()> {
        if self.fail {
            anyhow::bail!("Denied");
        }
        self.writes += 1;
        self.values[index(slot)] = v.map(str::to_string);
        Ok(())
    }
}
fn cmd() -> String {
    startup::command(std::path::Path::new(r"C:\Ferrum App\ferrum.exe")).unwrap()
}
#[test]
fn migration_requires_explicit_action_and_consolidates_exact_own_names() {
    let old = r#""D:\Old\spotlight-launcher-phase1-3.exe""#.to_string();
    let mut m = Memory {
        values: [Some(old), None],
        ..Default::default()
    };
    let state = inspect(&m, &cmd()).unwrap();
    assert!(!state.enabled);
    assert!(state.removable);
    assert!(state.message.contains("Legacy"));
    assert_eq!(m.writes, 0);
    change(&mut m, &cmd(), true, || Ok(())).unwrap();
    assert_eq!(m.values, [None, Some(cmd())]);
    assert!(inspect(&m, &cmd()).unwrap().enabled);
    change(&mut m, &cmd(), false, || Ok(())).unwrap();
    assert_eq!(m.values, [None, None]);
    assert_eq!(Slot::Ferrum.name(), "Ferrum");
    assert_eq!(Slot::Legacy.name(), "SpotlightLauncherPrototype");
}
#[test]
fn collisions_and_failed_migrations_preserve_old_settings() {
    let old = Some(r#""D:\Old\spotlight-launcher-phase1-3.exe""#.to_string());
    let mut m = Memory {
        values: [old.clone(), None],
        ..Default::default()
    };
    assert!(change(&mut m, &cmd(), true, || anyhow::bail!("Config locked")).is_err());
    assert_eq!(m.values, [old.clone(), None]);
    m.fail = true;
    assert!(change(&mut m, &cmd(), true, || panic!()).is_err());
    assert_eq!(m.values, [old, None]);
    m.fail = false;
    m.values[1] = Some(r#""C:\Elsewhere\unrelated.exe""#.into());
    let before = m.values.clone();
    let writes = m.writes;
    assert!(inspect(&m, &cmd()).is_err());
    assert!(change(&mut m, &cmd(), false, || panic!()).is_err());
    assert_eq!(m.values, before);
    assert_eq!(m.writes, writes);
    m.values = [
        Some(r#""C:\Elsewhere\spotlight-launcher-phase-malicious.exe""#.into()),
        None,
    ];
    assert!(inspect(&m, &cmd()).is_err());
}

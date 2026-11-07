use spotlight_prototype::startup::*;
use std::path::Path;
#[derive(Default)]
struct Memory {
    value: Option<String>,
    fail: bool,
    writes: Vec<Option<String>>,
}
impl Backend for Memory {
    fn read(&self) -> anyhow::Result<Option<String>> {
        Ok(self.value.clone())
    }
    fn write(&mut self, value: Option<&str>) -> anyhow::Result<()> {
        if self.fail {
            anyhow::bail!("Registry denied");
        }
        self.value = value.map(String::from);
        self.writes.push(self.value.clone());
        Ok(())
    }
}
fn expected() -> String {
    command(Path::new(
        r"C:\Program Files\Spotlight\spotlight-launcher-phase1-3.exe",
    ))
    .unwrap()
}
#[test]
fn quoted_command_and_own_entry_only() {
    let cmd = expected();
    assert_eq!(
        cmd,
        r#""C:\Program Files\Spotlight\spotlight-launcher-phase1-3.exe""#
    );
    assert_eq!(VALUE_NAME, "SpotlightLauncherPrototype");
    assert!(RUN_KEY.ends_with("\\Run"));
    assert!(command(Path::new("relative.exe")).is_err());
    assert!(command(Path::new(r"C:\bad\file.cmd")).is_err());
    let mut b = Memory::default();
    assert_eq!(status(b.read().unwrap().as_deref(), &cmd), Status::Off);
    change(&mut b, &cmd, true, || Ok(())).unwrap();
    assert_eq!(status(b.value.as_deref(), &cmd), Status::Enabled);
    change(&mut b, &cmd, false, || Ok(())).unwrap();
    assert_eq!(b.value, None);
    assert_eq!(b.writes.last(), Some(&None));
}
#[test]
fn stale_registration_is_reported_and_can_be_explicitly_repaired() {
    let old = command(Path::new(r"D:\Old\spotlight-launcher-phase1-2.exe")).unwrap();
    let cmd = expected();
    let mut b = Memory {
        value: Some(old.clone()),
        ..Default::default()
    };
    assert_eq!(status(b.value.as_deref(), &cmd), Status::Stale(old));
    change(&mut b, &cmd, true, || Ok(())).unwrap();
    assert_eq!(b.value, Some(cmd.clone()));
    b.value = Some(r#""C:\Other\unrelated.exe""#.into());
    assert_eq!(status(b.value.as_deref(), &cmd), Status::Foreign);
    assert!(change(&mut b, &cmd, false, || panic!()).is_err());
    assert_eq!(b.writes.len(), 1);
}
#[test]
fn denied_write_verification_failure_and_save_failure_never_claim_success() {
    let cmd = expected();
    let mut b = Memory {
        fail: true,
        ..Default::default()
    };
    assert!(change(&mut b, &cmd, true, || panic!()).is_err());
    assert_eq!(b.value, None);
    b.fail = false;
    assert!(change(&mut b, &cmd, true, || anyhow::bail!("Config denied")).is_err());
    assert_eq!(b.value, None);
    struct Ignored;
    impl Backend for Ignored {
        fn read(&self) -> anyhow::Result<Option<String>> {
            Ok(None)
        }
        fn write(&mut self, _: Option<&str>) -> anyhow::Result<()> {
            Ok(())
        }
    }
    assert!(change(&mut Ignored, &cmd, true, || panic!()).is_err());
}

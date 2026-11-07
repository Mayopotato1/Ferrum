//! Only Settings' explicit user action calls `change`. Tests inject a memory backend.
use anyhow::{Context, Result, bail};
use std::path::Path;
pub const VALUE_NAME: &str = "SpotlightLauncherPrototype";
pub const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
pub trait Backend {
    fn read(&self) -> Result<Option<String>>;
    fn write(&mut self, command: Option<&str>) -> Result<()>;
}
#[derive(Debug, PartialEq, Eq)]
pub enum Status {
    Off,
    Enabled,
    Stale(String),
    Foreign,
}
pub fn command(exe: &Path) -> Result<String> {
    let text = exe
        .to_str()
        .context("Executable path is not valid Unicode")?;
    if !exe.is_absolute()
        || text.contains(['"', '\r', '\n', '\0'])
        || exe
            .extension()
            .is_none_or(|e| !e.eq_ignore_ascii_case("exe"))
    {
        bail!("Startup needs an absolute .exe path without quotes or control characters");
    }
    let command = format!("\"{text}\"");
    if command.encode_utf16().count() > 260 {
        bail!("Startup command exceeds Windows Run's 260-character limit");
    }
    Ok(command)
}
fn own_command(value: &str) -> bool {
    // Only our exact registration name is read; do not overwrite a foreign
    // payload even if something else has occupied that name.
    let Some(path) = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')) else {
        return false;
    };
    let p = Path::new(path);
    p.is_absolute()
        && p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
            let n = n.to_ascii_lowercase();
            n == "spotlight-launcher.exe"
                || n.starts_with("spotlight-launcher-phase") && n.ends_with(".exe")
        })
        && !path.contains('"')
}
pub fn status(value: Option<&str>, expected: &str) -> Status {
    match value {
        None => Status::Off,
        Some(v) if v == expected => Status::Enabled,
        Some(v) if own_command(v) => Status::Stale(v.into()),
        Some(_) => Status::Foreign,
    }
}
/// Register/remove, verify by reading back, then persist. On failure restore the
/// prior payload and surface any rollback failure instead of claiming success.
pub fn change<B: Backend>(
    backend: &mut B,
    expected: &str,
    enabled: bool,
    save: impl FnOnce() -> Result<()>,
) -> Result<()> {
    let before = backend.read()?;
    if status(before.as_deref(), expected) == Status::Foreign {
        bail!("The startup value name contains an unrecognized command. It was left untouched.");
    }
    let desired = enabled.then_some(expected);
    let result = (|| -> Result<()> {
        backend.write(desired)?;
        if backend.read()?.as_deref() != desired {
            bail!("Windows did not retain the requested startup registration");
        }
        save()
    })();
    if let Err(error) = result {
        let rollback = backend.write(before.as_deref()).and_then(|()| {
            if backend.read()? != before {
                bail!("Startup rollback did not verify");
            }
            Ok(())
        });
        return match rollback { Ok(())=>Err(error), Err(rollback)=>Err(error.context(format!("Startup rollback also failed: {rollback:#}. Reopen Settings to inspect actual status."))) };
    }
    Ok(())
}
pub struct Registry;
struct Key(windows::Win32::System::Registry::HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::System::Registry::RegCloseKey(self.0);
        }
    }
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
pub struct NamedValue(pub(crate) &'static str);
impl Backend for Registry {
    fn read(&self) -> Result<Option<String>> {
        NamedValue(VALUE_NAME).read()
    }
    fn write(&mut self, command: Option<&str>) -> Result<()> {
        NamedValue(VALUE_NAME).write(command)
    }
}
impl Backend for NamedValue {
    fn read(&self) -> Result<Option<String>> {
        use windows::{
            Win32::{Foundation::*, System::Registry::*},
            core::PCWSTR,
        };
        let sub = wide(RUN_KEY);
        let name = wide(self.0);
        let mut bytes = 0;
        unsafe {
            let result = RegGetValueW(
                HKEY_CURRENT_USER,
                PCWSTR(sub.as_ptr()),
                PCWSTR(name.as_ptr()),
                RRF_RT_REG_SZ,
                None,
                None,
                Some(&mut bytes),
            );
            if result == ERROR_FILE_NOT_FOUND {
                return Ok(None);
            }
            result.ok()?;
            if bytes > 8192 {
                bail!("Startup value is unexpectedly large; it was not modified");
            }
            let mut value = vec![0u16; bytes as usize / 2 + 1];
            RegGetValueW(
                HKEY_CURRENT_USER,
                PCWSTR(sub.as_ptr()),
                PCWSTR(name.as_ptr()),
                RRF_RT_REG_SZ,
                None,
                Some(value.as_mut_ptr().cast()),
                Some(&mut bytes),
            )
            .ok()?;
            let length = value.iter().position(|c| *c == 0).unwrap_or(value.len());
            Ok(Some(String::from_utf16(&value[..length])?))
        }
    }
    fn write(&mut self, command: Option<&str>) -> Result<()> {
        use windows::{
            Win32::{Foundation::*, System::Registry::*},
            core::PCWSTR,
        };
        let sub = wide(RUN_KEY);
        let name = wide(self.0);
        let mut key = HKEY::default();
        unsafe {
            if command.is_some() {
                RegCreateKeyExW(
                    HKEY_CURRENT_USER,
                    PCWSTR(sub.as_ptr()),
                    None,
                    PCWSTR::null(),
                    REG_OPTION_NON_VOLATILE,
                    KEY_SET_VALUE,
                    None,
                    &mut key,
                    None,
                )
                .ok()?;
            } else {
                let result = RegOpenKeyExW(
                    HKEY_CURRENT_USER,
                    PCWSTR(sub.as_ptr()),
                    None,
                    KEY_SET_VALUE,
                    &mut key,
                );
                if result == ERROR_FILE_NOT_FOUND {
                    return Ok(());
                }
                result.ok()?;
            }
            let key = Key(key);
            if let Some(command) = command {
                let value = wide(command);
                let bytes: Vec<u8> = value.iter().flat_map(|u| u.to_le_bytes()).collect();
                RegSetValueExW(key.0, PCWSTR(name.as_ptr()), None, REG_SZ, Some(&bytes)).ok()?;
            } else {
                let result = RegDeleteValueW(key.0, PCWSTR(name.as_ptr()));
                if result != ERROR_FILE_NOT_FOUND {
                    result.ok()?;
                }
            }
        }
        Ok(())
    }
}

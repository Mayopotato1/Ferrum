use crate::{
    discovery,
    model::{Action, Item},
};
use anyhow::{Result, bail, ensure};
use windows::{
    Win32::{
        System::Com::*,
        UI::{Shell::*, WindowsAndMessaging::SW_SHOWNORMAL},
    },
    core::{HSTRING, PCWSTR},
};

#[derive(Debug, PartialEq, Eq)]
pub enum Prepared {
    Shell(String),
    Packaged(String),
}
pub fn prepare(item: &Item) -> Result<Prepared> {
    ensure!(item.available, "Action unavailable for {}", item.name);
    match &item.action {
        Action::Shortcut {
            path,
            target,
            arguments,
            working_dir,
        } => {
            ensure!(path.is_file(), "Shortcut no longer exists");
            let _com = discovery::Com::new()?;
            let current = discovery::shortcut(path)?;
            if let Action::Shortcut {
                target: t,
                arguments: a,
                working_dir: w,
                ..
            } = current.action
            {
                ensure!(
                    t == *target && a == *arguments && w == *working_dir,
                    "Shortcut changed; refresh before opening"
                );
            }
            ensure!(current.available, "Shortcut target is missing");
            Ok(Prepared::Shell(path.to_string_lossy().into()))
        }
        Action::Packaged { aumid } => {
            ensure!(
                aumid.contains('!') && !aumid.contains(['\0', '\\', '/']),
                "Invalid packaged app identity"
            );
            let _com = discovery::Com::new()?;
            let _: IShellItem = unsafe {
                SHCreateItemFromParsingName(
                    &HSTRING::from(format!("shell:AppsFolder\\{aumid}")),
                    None,
                )?
            };
            Ok(Prepared::Packaged(aumid.clone()))
        }
        Action::Steam {
            app_id,
            manifest,
            install_dir,
        } => {
            ensure!(
                *app_id > 0 && discovery::local_path(manifest),
                "Steam manifest unavailable"
            );
            let text = std::fs::read_to_string(manifest)?;
            let current = discovery::steam_manifest(&text, manifest, manifest.parent().unwrap())?;
            ensure!(
                current.available && current.action == item.action && install_dir.is_dir(),
                "Steam installation changed; refresh"
            );
            Ok(Prepared::Shell(format!("steam://rungameid/{app_id}")))
        }
        Action::Folder(p) => {
            ensure!(
                discovery::local_path(p) && p.is_dir(),
                "Folder missing, remote, or unavailable"
            );
            Ok(Prepared::Shell(p.to_string_lossy().into()))
        }
        Action::Document(p) => {
            ensure!(
                discovery::local_path(p) && p.is_file() && discovery::safe_document(p),
                "Document missing or format not permitted"
            );
            Ok(Prepared::Shell(p.to_string_lossy().into()))
        }
        Action::Website(s) => {
            let u = url::Url::parse(s)?;
            ensure!(
                u.scheme() == "https"
                    && u.host_str().is_some()
                    && u.username().is_empty()
                    && u.password().is_none(),
                "Invalid HTTPS website action"
            );
            Ok(Prepared::Shell(s.clone()))
        }
        Action::WebSearch(s) => Ok(Prepared::Shell(crate::web::validate_search_url(s)?.into())),
        Action::OpenUrl(s) => Ok(Prepared::Shell(crate::web::validate_open_url(s)?.into())),
        Action::Unavailable(reason) => bail!("{reason}"),
    }
}
// The GUI's explicit Enter/click handler is the only caller of execute.
pub fn execute(item: &Item) -> Result<()> {
    let p = prepare(item)?;
    let _com = discovery::Com::new()?;
    unsafe {
        match p {
            Prepared::Shell(target) => {
                let result = ShellExecuteW(
                    None,
                    windows::core::w!("open"),
                    &HSTRING::from(target),
                    PCWSTR::null(),
                    PCWSTR::null(),
                    SW_SHOWNORMAL,
                );
                ensure!(
                    result.0 as isize > 32,
                    "Windows Shell rejected action (code {})",
                    result.0 as isize
                );
            }
            Prepared::Packaged(id) => {
                let manager: IApplicationActivationManager =
                    CoCreateInstance(&ApplicationActivationManager, None, CLSCTX_INPROC_SERVER)?;
                manager.ActivateApplication(&HSTRING::from(id), PCWSTR::null(), AO_NONE)?;
            }
        }
    }
    Ok(())
}

use std::{env, path::PathBuf, process::Command};
#[path = "packaging/version.rs"]
mod release_version;
fn main() {
    if env::var_os("CARGO_FEATURE_UPDATE_FIXTURE").is_some() {
        assert!(
            env::var("PROFILE").as_deref() != Ok("release"),
            "update-fixture is forbidden in release builds"
        );
        assert!(
            env::var_os("FERRUM_UPDATE_OWNER").is_some()
                && env::var_os("FERRUM_UPDATE_REPO").is_some(),
            "Fixture builds require explicit test source identity"
        );
    }
    for key in [
        "FERRUM_UPDATE_OWNER",
        "FERRUM_UPDATE_REPO",
        "FERRUM_UPDATE_CHANNEL",
    ] {
        println!("cargo:rerun-if-env-changed={key}");
    }
    println!("cargo:rerun-if-changed=packaging/update-source.json");
    let pinned: serde_json::Value = serde_json::from_slice(
        &std::fs::read("packaging/update-source.json").expect("Missing pinned update source"),
    )
    .expect("Invalid pinned update source JSON");
    let configured = pinned["configured"]
        .as_bool()
        .expect("Missing configured flag");
    let mut owner = if configured {
        pinned["owner"].as_str().expect("Missing owner")
    } else {
        ""
    }
    .to_owned();
    let mut repo = if configured {
        pinned["repository"].as_str().expect("Missing repository")
    } else {
        ""
    }
    .to_owned();
    let mut channel = pinned["channel"]
        .as_str()
        .expect("Missing channel")
        .to_owned();
    // Production builds cannot be silently repointed by a caller's environment.
    // Only explicitly requested debug fixture builds may override the test identity.
    for (key, value) in [
        ("FERRUM_UPDATE_OWNER", &mut owner),
        ("FERRUM_UPDATE_REPO", &mut repo),
        ("FERRUM_UPDATE_CHANNEL", &mut channel),
    ] {
        if let Ok(requested) = env::var(key) {
            if env::var_os("CARGO_FEATURE_UPDATE_FIXTURE").is_some() {
                *value = requested;
            } else {
                assert_eq!(
                    &requested, value,
                    "{key} differs from the pinned update source"
                );
            }
        }
    }
    let valid = |s: &str| {
        !s.is_empty()
            && s.len() <= 100
            && s != "."
            && s != ".."
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
    };
    assert!(
        (owner.is_empty() && repo.is_empty()) || (valid(&owner) && valid(&repo)),
        "Set both exact update owner and repo, or neither"
    );
    assert!(
        channel == "beta" || channel == "stable",
        "Update channel must be beta or stable"
    );
    println!("cargo:rustc-env=FERRUM_UPDATE_OWNER={owner}");
    println!("cargo:rustc-env=FERRUM_UPDATE_REPO={repo}");
    println!("cargo:rustc-env=FERRUM_UPDATE_CHANNEL={channel}");
    println!("cargo:rerun-if-changed=resources/ferrum.rc");
    println!("cargo:rerun-if-changed=resources/ferrum.ico");
    println!("cargo:rerun-if-changed=packaging/version.rs");
    println!("cargo:rerun-if-env-changed=RC");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let arch = match env::var("CARGO_CFG_TARGET_ARCH").unwrap().as_str() {
        "aarch64" => "arm64",
        "x86" => "x86",
        _ => "x64",
    };
    let compiler=env::var_os("RC").map(PathBuf::from).or_else(||{
        let root=PathBuf::from(env::var_os("ProgramFiles(x86)")?).join("Windows Kits/10/bin");
        let mut versions:Vec<_>=root.read_dir().ok()?.flatten().map(|e|e.path()).filter(|p|p.join(arch).join("rc.exe").is_file()).collect();
        versions.sort_by_key(|p|p.file_name().unwrap().to_string_lossy().split('.').filter_map(|v|v.parse::<u32>().ok()).collect::<Vec<_>>());
        versions.pop().map(|p|p.join(arch).join("rc.exe"))
    }).expect("Windows SDK rc.exe not found. Use the installed MSVC/SDK environment or set RC to its path.");
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let output = out_dir.join("ferrum.res");
    let version = env::var("CARGO_PKG_VERSION").unwrap();
    let numeric = release_version::numeric(&version).expect("Unsupported Windows release version");
    let numeric = numeric.map(|v| v.to_string()).join(",");
    let dotted = numeric.replace(',', ".");
    let icon =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("resources/ferrum.ico");
    let source = std::fs::read_to_string("resources/ferrum.rc")
        .unwrap()
        .replace("@NUMERIC_VERSION@", &numeric)
        .replace("@DOTTED_VERSION@", &dotted)
        .replace("@SEMVER@", &version)
        .replace("@ICON_PATH@", &icon.to_string_lossy().replace('\\', "\\\\"));
    let rc = out_dir.join("ferrum.rc");
    std::fs::write(&rc, source).unwrap();
    let mut command = Command::new(compiler);
    command.arg("/nologo").arg("/fo").arg(&output).arg(rc);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    assert!(
        command
            .status()
            .expect("Failed to run Windows resource compiler")
            .success(),
        "Ferrum VERSIONINFO compilation failed"
    );
    println!("cargo:rustc-link-arg-bin=ferrum={}", output.display());
}

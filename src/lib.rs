pub mod actions;
pub mod cache;
pub mod composition;
pub mod config;
pub mod diagnostics;
pub mod discovery;
pub mod hotkeys;
pub mod instance;
pub mod model;
pub mod preferences;
pub mod query;
#[path = "../packaging/version.rs"]
pub mod release_version;
pub mod search;
pub mod startup;
pub mod startup_migration;
pub mod validation;
pub mod web;

pub mod updates;

#[cfg(all(feature = "update-fixture", not(debug_assertions)))]
compile_error!("update-fixture requires a debug build and is forbidden for distribution");

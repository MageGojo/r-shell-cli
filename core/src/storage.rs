use std::{fs, path::PathBuf};

use anyhow::{Context, Result};

use crate::model::PersistedWorkspace;

fn workspace_path() -> Result<PathBuf> {
    let data_dir = dirs::data_local_dir().context("cannot resolve the local data directory")?;
    Ok(data_dir.join("r-shell").join("workspace.json"))
}

pub fn load_workspace() -> PersistedWorkspace {
    let Ok(path) = workspace_path() else {
        return PersistedWorkspace::default();
    };
    let Ok(contents) = fs::read_to_string(path) else {
        return PersistedWorkspace::default();
    };

    parse_workspace_json(&contents).unwrap_or_default()
}

pub fn parse_workspace_json(contents: &str) -> Result<PersistedWorkspace> {
    Ok(serde_json::from_str::<PersistedWorkspace>(contents)?.normalized())
}

pub fn workspace_to_json(workspace: &PersistedWorkspace) -> Result<String> {
    Ok(serde_json::to_string_pretty(workspace)?)
}

pub fn save_workspace(workspace: &PersistedWorkspace) -> Result<()> {
    let path = workspace_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create {}", parent.display()))?;
        // The workspace may hold plaintext credentials, so keep the directory
        // private to the current user (0700).
        restrict_dir_permissions(parent);
    }

    let json = workspace_to_json(workspace)?;
    fs::write(&path, json).with_context(|| format!("cannot write {}", path.display()))?;
    // Restrict the file to the owner only (0600) after writing.
    restrict_file_permissions(&path);
    Ok(())
}

#[cfg(unix)]
fn restrict_file_permissions(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
}

#[cfg(unix)]
fn restrict_dir_permissions(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o700));
}

#[cfg(not(unix))]
fn restrict_file_permissions(_path: &std::path::Path) {}

#[cfg(not(unix))]
fn restrict_dir_permissions(_path: &std::path::Path) {}

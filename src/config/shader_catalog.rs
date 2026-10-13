use std::path::PathBuf;

use super::toml::{config_path, expand_tilde};

pub const WINDOW_PORTAL_SRC: &str = include_str!("../shaders/window_portal.glsl");
pub const WINDOW_FIRE_SRC: &str = include_str!("../shaders/window_fire.glsl");
pub const WINDOW_TV_SRC: &str = include_str!("../shaders/window_tv.glsl");
pub const WINDOW_DOOM_SRC: &str = include_str!("../shaders/window_doom.glsl");
pub const WINDOW_ANOMALY_SRC: &str = include_str!("../shaders/window_anomaly.glsl");

/// Directory where custom and template window shaders are stored (`~/.config/driftwm/shaders`).
pub fn shaders_dir() -> PathBuf {
    config_path()
        .parent()
        .map(|p| p.join("shaders"))
        .unwrap_or_else(|| PathBuf::from("shaders"))
}

/// Ensure `~/.config/driftwm/shaders` exists and has the 5 default preset shaders
/// so users have clear working examples to customize.
pub fn ensure_default_shaders_exported() {
    let dir = shaders_dir();
    if !dir.exists() {
        if let Err(e) = std::fs::create_dir_all(&dir) {
            tracing::warn!("Failed to create shaders directory {dir:?}: {e}");
            return;
        }
    }

    let defaults = [
        ("portal.glsl", WINDOW_PORTAL_SRC),
        ("fire.glsl", WINDOW_FIRE_SRC),
        ("tv.glsl", WINDOW_TV_SRC),
        ("doom.glsl", WINDOW_DOOM_SRC),
        ("anomaly.glsl", WINDOW_ANOMALY_SRC),
    ];

    for (name, src) in defaults {
        let path = dir.join(name);
        if !path.exists() {
            if let Err(e) = std::fs::write(&path, src) {
                tracing::warn!("Failed to write default shader template {path:?}: {e}");
            }
        }
    }
}

/// Check if a user override exists on disk for a preset (e.g. `shaders/portal.glsl`).
pub fn preset_disk_override(preset: &str) -> Option<PathBuf> {
    let dir = shaders_dir();
    let file = format!("{preset}.glsl");
    let candidate = dir.join(&file);
    if candidate.is_file() {
        return Some(candidate);
    }
    None
}

/// Resolve a shader spec from config to a file path.
/// Tries:
/// 1. Expanded direct path (if contains `/`, starts with `~`, or ends with `.glsl`).
/// 2. `~/.config/driftwm/shaders/<name>.glsl`
/// 3. `~/.config/driftwm/<name>.glsl`
/// 4. Fallback to `~/.config/driftwm/shaders/<name>.glsl` even if not existing yet.
pub fn resolve_shader_path(spec: &str) -> PathBuf {
    let trimmed = spec.trim();
    if trimmed.starts_with('/')
        || trimmed.starts_with("~/")
        || trimmed.starts_with("./")
        || trimmed.ends_with(".glsl")
    {
        let expanded = expand_tilde(trimmed);
        let path = PathBuf::from(expanded);
        if path.is_file() {
            return path;
        }
        // If not found directly, check inside shaders_dir if it's just a filename
        if !trimmed.contains('/') {
            let in_shaders = shaders_dir().join(trimmed);
            if in_shaders.is_file() {
                return in_shaders;
            }
        }
        return path;
    }

    // Try in shaders directory: <spec>.glsl
    let in_shaders = shaders_dir().join(format!("{trimmed}.glsl"));
    if in_shaders.is_file() {
        return in_shaders;
    }

    // Try in config directory directly: <spec>.glsl
    if let Some(parent) = config_path().parent() {
        let in_config = parent.join(format!("{trimmed}.glsl"));
        if in_config.is_file() {
            return in_config;
        }
    }

    // Return in_shaders as canonical target path
    in_shaders
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_shader_path_presets() {
        let path = resolve_shader_path("my_effect");
        assert!(path.to_string_lossy().ends_with("my_effect.glsl"));
    }

    #[test]
    fn test_resolve_shader_path_direct() {
        let path = resolve_shader_path("/tmp/some_shader.glsl");
        assert_eq!(path, PathBuf::from("/tmp/some_shader.glsl"));
    }
}

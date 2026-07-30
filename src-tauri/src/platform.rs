use std::ffi::OsStr;
use std::path::Path;

const NVIDIA_DRIVER_VERSION_PATH: &str = "/proc/driver/nvidia/version";
const NVIDIA_EXPLICIT_SYNC_VAR: &str = "__NV_DISABLE_EXPLICIT_SYNC";

pub fn apply_nvidia_wayland_workaround() -> bool {
    let wayland_display = std::env::var_os("WAYLAND_DISPLAY");
    let explicit_sync_setting = std::env::var_os(NVIDIA_EXPLICIT_SYNC_VAR);
    let nvidia_driver_present = Path::new(NVIDIA_DRIVER_VERSION_PATH).exists();

    if should_disable_nvidia_explicit_sync(
        wayland_display.as_deref(),
        nvidia_driver_present,
        explicit_sync_setting.as_deref(),
    ) {
        // Must happen before Tauri initializes GTK/EGL; a pre-set value is an explicit override.
        std::env::set_var(NVIDIA_EXPLICIT_SYNC_VAR, "1");
        return true;
    }

    false
}

fn should_disable_nvidia_explicit_sync(
    wayland_display: Option<&OsStr>,
    nvidia_driver_present: bool,
    explicit_sync_setting: Option<&OsStr>,
) -> bool {
    wayland_display.is_some() && nvidia_driver_present && explicit_sync_setting.is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_to_nvidia_wayland_without_an_override() {
        assert!(should_disable_nvidia_explicit_sync(
            Some(OsStr::new("wayland-0")),
            true,
            None,
        ));
    }

    #[test]
    fn does_not_apply_without_wayland() {
        assert!(!should_disable_nvidia_explicit_sync(None, true, None));
    }

    #[test]
    fn does_not_apply_without_nvidia() {
        assert!(!should_disable_nvidia_explicit_sync(
            Some(OsStr::new("wayland-0")),
            false,
            None,
        ));
    }

    #[test]
    fn preserves_any_existing_explicit_sync_setting() {
        for value in ["0", "1", ""] {
            assert!(!should_disable_nvidia_explicit_sync(
                Some(OsStr::new("wayland-0")),
                true,
                Some(OsStr::new(value)),
            ));
        }
    }
}

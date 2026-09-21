//! Shared desktop disc boot path for the launcher and headless CLI.
use std::path::{Path, PathBuf};

use emulator_core::{fast_boot_disc_with_hle, warm_bios_for_disc_fast_boot, Bus, Cpu};
use psoxide_settings::Settings;
use psx_iso::Disc;

pub(crate) fn resolve_bios_path(settings: &Settings) -> Result<PathBuf, String> {
    let configured = settings.paths.bios.trim();
    resolve_bios_path_with_env(configured, std::env::var_os("PSOXIDE_BIOS").as_deref())
}

fn resolve_bios_path_with_env(
    configured: &str,
    fallback: Option<&std::ffi::OsStr>,
) -> Result<PathBuf, String> {
    if !configured.is_empty() {
        Ok(PathBuf::from(configured))
    } else if let Some(path) = fallback.filter(|path| !path.is_empty()) {
        Ok(PathBuf::from(path))
    } else {
        Err("BIOS path is not configured. Open Settings and choose a BIOS image, use --bios PATH, or export PSOXIDE_BIOS. Homebrew discs can use --embedded-playtest.".into())
    }
}

pub(crate) fn configured_bus(settings: &Settings) -> Result<Bus, String> {
    let path = resolve_bios_path(settings)?;
    let bytes =
        std::fs::read(&path).map_err(|error| format!("BIOS {}: {error}", path.display()))?;
    Bus::new(bytes).map_err(|error| format!("BIOS rejected: {error}"))
}

/// Match the standalone emulator: warm real firmware before fast boot,
/// preserving normal BIOS boot as the fallback when direct boot fails.
pub(crate) fn boot_disc(
    bus: &mut Bus,
    cpu: &mut Cpu,
    disc: &Disc,
    path: &Path,
    fast: bool,
    warmup_steps: u64,
) -> &'static str {
    if !fast {
        return "BIOS boot";
    }
    bus.cdrom.insert_disc(Some(disc.clone()));
    if let Err(error) = warm_bios_for_disc_fast_boot(bus, cpu, warmup_steps) {
        eprintln!(
            "[frontend] BIOS warmup failed for {} ({error:?}); falling back to BIOS boot",
            path.display()
        );
        return "BIOS boot";
    }
    match fast_boot_disc_with_hle(bus, cpu, disc, false) {
        Ok(info) => {
            eprintln!(
                "[frontend] warm-fast-booted {} via {} entry=0x{:08x}",
                path.display(),
                info.boot_path,
                info.initial_pc
            );
            "warm fast boot"
        }
        Err(error) => {
            eprintln!(
                "[frontend] fast boot unavailable for {} ({error:?}); falling back to BIOS boot",
                path.display()
            );
            "BIOS boot"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn missing_or_empty_bios_configuration_is_actionable() {
        for fallback in [None, Some(OsStr::new(""))] {
            let error = resolve_bios_path_with_env("", fallback).unwrap_err();
            assert!(error.contains("Settings"));
            assert!(error.contains("PSOXIDE_BIOS"));
        }
    }

    #[test]
    fn missing_bios_file_reports_its_path() {
        let mut settings = Settings::default();
        let path = std::env::temp_dir().join(format!(
            "psoxide-absent-firmware-{}.bin",
            std::process::id()
        ));
        settings.paths.bios = path.display().to_string();
        let error = match configured_bus(&settings) {
            Ok(_) => panic!("a nonexistent firmware file must fail before emulation"),
            Err(error) => error,
        };
        assert!(error.contains("BIOS"));
        assert!(error.contains(&path.display().to_string()));
    }

    #[test]
    fn settings_bios_takes_precedence_over_environment() {
        assert_eq!(
            resolve_bios_path_with_env("configured.bin", Some(OsStr::new("env.bin"))).unwrap(),
            PathBuf::from("configured.bin")
        );
        assert_eq!(
            resolve_bios_path_with_env("", Some(OsStr::new("env.bin"))).unwrap(),
            PathBuf::from("env.bin")
        );
    }
}

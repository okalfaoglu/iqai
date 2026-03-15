//! IQAI loglama modülü – info, debug, warning, error, critical.
//!
//! Seviyeler: `trace!`, `debug!`, `info!`, `warn!`, `error!`, `critical!`.
//! Başlatma: config.json "logging" bölümünden veya varsayılan (info, console).

use crate::app_config::{LogTarget, LoggingConfig};
use flexi_logger::{Duplicate, FileSpec, Logger};
use std::path::Path;

pub use log::{debug, error, info, trace, warn};

/// Config'ten loglama başlat. Config yoksa veya logging bölümü yoksa: info, console.
/// RUST_LOG env varsa onu kullanır (flexi_logger try_with_env_or_str).
pub fn init_from_config(config: Option<&LoggingConfig>) -> Result<(), flexi_logger::FlexiLoggerError> {
    let level = config
        .map(|c| c.level.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("info");
    let target = config.map(|c| c.target).unwrap_or(LogTarget::Console);
    let file_path = config
        .and_then(|c| c.file_path.as_deref())
        .filter(|s| !s.is_empty())
        .unwrap_or("iqai.log");

    let mut logger = Logger::try_with_str(level)?;

    match target {
        LogTarget::Console => {
            logger = logger.log_to_stderr();
        }
        LogTarget::File => {
            let spec = file_spec_from_path(file_path);
            logger = logger.log_to_file(spec);
        }
        LogTarget::Both => {
            let spec = file_spec_from_path(file_path);
            logger = logger
                .log_to_file(spec)
                .append()
                .duplicate_to_stderr(Duplicate::All);
        }
    }

    logger.start()?;
    Ok(())
}

fn file_spec_from_path(path: &str) -> FileSpec {
    let p = Path::new(path);
    if let (Some(parent), Some(name)) = (p.parent(), p.file_stem()) {
        let basename = name.to_string_lossy().into_owned();
        let suffix = p.extension().and_then(|e| e.to_str()).unwrap_or("log");
        return FileSpec::default()
            .directory(parent)
            .basename(basename)
            .suffix(suffix);
    }
    FileSpec::default()
        .basename(path.trim_end_matches(".log"))
        .suffix("log")
}

/// Kritik hata – log seviyesi olarak `error` kullanılır.
#[macro_export]
macro_rules! critical {
    ($($t:tt)*) => {
        log::error!($($t)*)
    };
}

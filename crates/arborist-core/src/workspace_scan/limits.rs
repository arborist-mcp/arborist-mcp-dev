use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

pub const DEFAULT_WORKSPACE_MAX_FILES: usize = 20_000;
pub const MAX_WORKSPACE_SCAN_TIMEOUT_MS: u64 = 5 * 60 * 1_000;

#[derive(Debug, Clone, Copy)]
pub struct WorkspaceScanLimits {
    pub max_files: usize,
    pub max_file_bytes: Option<u64>,
    pub timeout_ms: Option<u64>,
}

impl Default for WorkspaceScanLimits {
    fn default() -> Self {
        Self {
            max_files: DEFAULT_WORKSPACE_MAX_FILES,
            max_file_bytes: None,
            timeout_ms: None,
        }
    }
}

impl WorkspaceScanLimits {
    pub fn with_max_files(max_files: usize) -> Self {
        Self {
            max_files,
            ..Self::default()
        }
    }

    pub fn with_timeout_ms(timeout_ms: u64) -> Self {
        Self {
            timeout_ms: Some(timeout_ms),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct WorkspaceScanDeadline {
    pub(crate) deadline: Option<Instant>,
    pub(crate) timeout_ms: Option<u64>,
}

impl WorkspaceScanDeadline {
    pub(crate) fn new(limits: WorkspaceScanLimits) -> Result<Self> {
        validate_workspace_scan_limits(limits)?;
        Ok(Self {
            deadline: limits
                .timeout_ms
                .map(|timeout_ms| Instant::now() + Duration::from_millis(timeout_ms)),
            timeout_ms: limits.timeout_ms,
        })
    }

    pub(crate) fn check(&self, phase: &str) -> Result<()> {
        if self
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            bail!(
                "workspace scan timeout exceeded during {phase}: timeout_ms={}",
                self.timeout_ms.unwrap_or_default(),
            );
        }
        Ok(())
    }
}

pub(crate) fn validate_workspace_scan_limits(limits: WorkspaceScanLimits) -> Result<()> {
    if limits.max_files == 0 {
        bail!("invalid workspace scan max_files: value must be greater than zero");
    }
    if limits.max_file_bytes == Some(0) {
        bail!("invalid workspace scan max_file_bytes: value must be greater than zero");
    }
    if limits.timeout_ms == Some(0) {
        bail!("invalid workspace scan timeout_ms: value must be greater than zero");
    }
    if limits
        .timeout_ms
        .is_some_and(|timeout_ms| timeout_ms > MAX_WORKSPACE_SCAN_TIMEOUT_MS)
    {
        bail!(
            "invalid workspace scan timeout_ms: value must not exceed {}",
            MAX_WORKSPACE_SCAN_TIMEOUT_MS,
        );
    }
    Ok(())
}

pub(crate) fn validate_source_file_size(path: &Path, limits: WorkspaceScanLimits) -> Result<()> {
    let Some(max_file_bytes) = limits.max_file_bytes else {
        return Ok(());
    };

    let metadata = fs::metadata(path)
        .with_context(|| format!("failed to inspect source file {}", path.display()))?;
    validate_source_file_metadata(path, &metadata, max_file_bytes)
}

pub(super) fn validate_source_file_metadata(
    path: &Path,
    metadata: &fs::Metadata,
    max_file_bytes: u64,
) -> Result<()> {
    if metadata.len() > max_file_bytes {
        bail!(
            "workspace scan source file too large at {}: size_bytes={} max_file_bytes={}",
            path.display(),
            metadata.len(),
            max_file_bytes,
        );
    }
    Ok(())
}

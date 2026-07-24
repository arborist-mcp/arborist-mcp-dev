use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};

pub fn read_source(path: &Path) -> Result<String> {
    fs::read_to_string(path)
        .with_context(|| format!("failed to read source file {}", path.display()))
}

pub(crate) fn write_source_atomic(path: &Path, source: &str) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| anyhow!("failed to resolve parent directory for {}", path.display()))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("failed to resolve file name for {}", path.display()))?;

    for attempt in 0..100usize {
        let temp_path = parent.join(format!(
            ".{file_name}.arborist-tmp-{}-{attempt}",
            std::process::id()
        ));
        let mut temp_file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("failed to create temporary file {}", temp_path.display())
                });
            }
        };

        let replace_result = (|| -> Result<()> {
            temp_file
                .write_all(source.as_bytes())
                .with_context(|| format!("failed to write {}", temp_path.display()))?;
            temp_file
                .sync_all()
                .with_context(|| format!("failed to sync {}", temp_path.display()))?;
            drop(temp_file);
            replace_file_atomically(&temp_path, path).with_context(|| {
                format!(
                    "failed to replace {} with temporary file {}",
                    path.display(),
                    temp_path.display()
                )
            })?;
            Ok(())
        })();

        if replace_result.is_err() {
            let _ = fs::remove_file(&temp_path);
        }
        return replace_result;
    }

    bail!(
        "failed to allocate a temporary file name for atomic write to {}",
        path.display()
    );
}

#[cfg(unix)]
pub(super) fn replace_file_atomically(temp_path: &Path, path: &Path) -> std::io::Result<()> {
    fs::rename(temp_path, path)
}

#[cfg(windows)]
pub(super) fn replace_file_atomically(temp_path: &Path, path: &Path) -> std::io::Result<()> {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::{null, null_mut};

    if !path.exists() {
        return fs::rename(temp_path, path);
    }

    let replaced = path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let replacement = temp_path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn ReplaceFileW(
            lpReplacedFileName: *const u16,
            lpReplacementFileName: *const u16,
            lpBackupFileName: *const u16,
            dwReplaceFlags: u32,
            lpExclude: *mut c_void,
            lpReserved: *mut c_void,
        ) -> i32;
    }

    let replaced = unsafe {
        ReplaceFileW(
            replaced.as_ptr(),
            replacement.as_ptr(),
            null(),
            0,
            null_mut(),
            null_mut(),
        )
    };
    if replaced == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(any(unix, windows)))]
pub(super) fn replace_file_atomically(temp_path: &Path, path: &Path) -> std::io::Result<()> {
    fs::rename(temp_path, path)
}

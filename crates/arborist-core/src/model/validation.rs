use std::collections::BTreeSet;

use anyhow::{Result, bail};

use super::Position;

pub(crate) fn ensure_nonblank(value: &str, field: &str) -> Result<()> {
    if value.trim().is_empty() {
        bail!("invalid {field}: value must not be blank");
    }
    Ok(())
}

pub(crate) fn ensure_nonblank_strings(values: &[String], field: &str) -> Result<()> {
    if let Some(index) = values.iter().position(|value| value.trim().is_empty()) {
        bail!("invalid {field}[{index}]: value must not be blank");
    }
    Ok(())
}

pub(crate) fn ensure_unique_strings(values: &[String], field: &str) -> Result<()> {
    let mut seen = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        if !seen.insert(value.clone()) {
            bail!("invalid {field}[{index}]: duplicate values are not allowed");
        }
    }
    Ok(())
}

pub(crate) fn point_is_after(start: &Position, end: &Position) -> bool {
    start.row > end.row || (start.row == end.row && start.column > end.column)
}

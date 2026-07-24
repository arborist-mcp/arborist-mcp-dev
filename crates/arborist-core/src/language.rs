use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use tree_sitter::{Language, Parser, Tree};

use crate::model::LanguageId;

mod c;
mod paths;
mod positions;
mod tree;

pub use c::{
    C_FAMILY_HEADER_EXTENSIONS, C_HEADER_EXTENSIONS, C_SOURCE_EXTENSIONS, CPP_HEADER_EXTENSIONS,
    CPP_SOURCE_EXTENSIONS, c_companion_source_path, c_include_targets, c_local_include_targets,
    is_c_header_path, resolve_local_c_include,
};
pub(crate) use c::{c_include_targets_before, extension_case_candidates};
pub(crate) use paths::{ensure_path_inside_workspace, path_is_inside_workspace};
pub use paths::{normalize_absolute_path, normalize_path};
pub use positions::{offset_for_position, point_for_offset, position_from};
pub use tree::*;

pub struct ParsedDocument {
    pub language_id: LanguageId,
    pub tree: Tree,
}

pub fn supported_languages() -> Vec<&'static str> {
    vec!["python", "c", "cpp"]
}

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
fn replace_file_atomically(temp_path: &Path, path: &Path) -> std::io::Result<()> {
    fs::rename(temp_path, path)
}

#[cfg(windows)]
fn replace_file_atomically(temp_path: &Path, path: &Path) -> std::io::Result<()> {
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
fn replace_file_atomically(temp_path: &Path, path: &Path) -> std::io::Result<()> {
    fs::rename(temp_path, path)
}

pub fn parse_document(path: &Path, source: &str) -> Result<ParsedDocument> {
    let language_id = detect_language(path)?;
    let mut parser = parser_for_language(language_id)?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow!("failed to parse {}", path.display()))?;

    Ok(ParsedDocument { language_id, tree })
}

pub fn parser_for_language(language_id: LanguageId) -> Result<Parser> {
    let language = language_for_id(language_id);
    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .context("failed to configure parser language")?;
    Ok(parser)
}

pub fn detect_language(path: &Path) -> Result<LanguageId> {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("py") || ext.eq_ignore_ascii_case("pyi") => {
            Ok(LanguageId::Python)
        }
        Some(ext)
            if C_SOURCE_EXTENSIONS
                .iter()
                .any(|extension| ext.eq_ignore_ascii_case(extension))
                || C_HEADER_EXTENSIONS
                    .iter()
                    .any(|extension| ext.eq_ignore_ascii_case(extension)) =>
        {
            Ok(LanguageId::C)
        }
        Some(ext)
            if CPP_SOURCE_EXTENSIONS
                .iter()
                .any(|extension| ext.eq_ignore_ascii_case(extension))
                || CPP_HEADER_EXTENSIONS
                    .iter()
                    .any(|extension| ext.eq_ignore_ascii_case(extension)) =>
        {
            Ok(LanguageId::Cpp)
        }
        other => bail!(
            "unsupported file extension {:?} for {}",
            other,
            path.display()
        ),
    }
}

pub fn language_for_id(language_id: LanguageId) -> Language {
    match language_id {
        LanguageId::Python => tree_sitter_python::LANGUAGE.into(),
        LanguageId::C => tree_sitter_c::LANGUAGE.into(),
        LanguageId::Cpp => tree_sitter_cpp::LANGUAGE.into(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use tree_sitter::Point;

    use super::{
        c_companion_source_path, detect_language, is_c_header_path, normalize_absolute_path,
        offset_for_position, parse_document, point_for_offset, supported_languages,
    };
    use crate::model::{LanguageId, Position};

    #[test]
    fn detect_language_accepts_uppercase_extensions() {
        for (extension, expected_language) in [
            ("PY", LanguageId::Python),
            ("PYI", LanguageId::Python),
            ("C", LanguageId::C),
            ("H", LanguageId::C),
            ("CC", LanguageId::Cpp),
            ("CPP", LanguageId::Cpp),
            ("CXX", LanguageId::Cpp),
            ("C++", LanguageId::Cpp),
            ("TPP", LanguageId::Cpp),
            ("TCC", LanguageId::Cpp),
            ("IPP", LanguageId::Cpp),
            ("INL", LanguageId::Cpp),
            ("HPP", LanguageId::Cpp),
            ("HH", LanguageId::Cpp),
            ("HXX", LanguageId::Cpp),
            ("H++", LanguageId::Cpp),
        ] {
            assert_eq!(
                detect_language(Path::new(&format!("sample.{extension}"))).unwrap(),
                expected_language,
                "unexpected language for .{extension}",
            );
        }
    }

    #[test]
    fn supported_languages_reports_cpp() {
        assert_eq!(supported_languages(), vec!["python", "c", "cpp"]);
    }

    #[test]
    fn detect_language_reports_original_unsupported_extension() {
        let error = detect_language(Path::new("sample.TXT"))
            .expect_err("unsupported extensions should be reported");

        assert!(error.to_string().contains(r#"Some("TXT")"#));
    }

    #[test]
    fn c_header_detection_accepts_uppercase_extensions() {
        assert!(is_c_header_path(Path::new("sample.h")));
        assert!(is_c_header_path(Path::new("sample.H")));
        assert!(is_c_header_path(Path::new("sample.HPP")));
        assert!(is_c_header_path(Path::new("sample.HH")));
        assert!(!is_c_header_path(Path::new("sample.c")));
    }

    #[test]
    fn parse_document_uses_cpp_grammar_for_cpp_extensions() {
        let source = "class Counter { public: int value() const { return 1; } };";
        for extension in ["hpp", "tpp", "tcc", "ipp", "inl"] {
            let document =
                parse_document(Path::new(&format!("counter.{extension}")), source).unwrap();

            assert_eq!(document.language_id, LanguageId::Cpp);
            assert!(!document.tree.root_node().has_error());
        }
    }

    #[test]
    fn companion_c_source_prefers_header_case_style() {
        let dir = std::env::temp_dir().join(format!(
            "arborist-language-companion-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let uppercase_header = dir.join("helper.H");
        let uppercase_source = dir.join("helper.C");
        std::fs::write(&uppercase_header, "int helper(int value);\n").unwrap();
        std::fs::write(
            &uppercase_source,
            "int helper(int value) { return value + 1; }\n",
        )
        .unwrap();

        assert_eq!(
            c_companion_source_path(&uppercase_header).unwrap(),
            uppercase_source
        );

        let mixed_header = dir.join("mixed.HPP");
        let lowercase_source = dir.join("mixed.c");
        std::fs::write(&mixed_header, "int mixed(int value);\n").unwrap();
        std::fs::write(
            &lowercase_source,
            "int mixed(int value) { return value + 1; }\n",
        )
        .unwrap();

        assert_eq!(
            c_companion_source_path(&mixed_header).unwrap(),
            lowercase_source
        );

        let template_header = dir.join("template.hpp");
        let template_implementation = dir.join("template.tpp");
        std::fs::write(
            &template_header,
            "template <typename T> T value(T input);\n",
        )
        .unwrap();
        std::fs::write(
            &template_implementation,
            "template <typename T> T value(T input) { return input; }\n",
        )
        .unwrap();

        assert_eq!(c_companion_source_path(&template_header), None);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn normalize_absolute_path_rejects_empty_paths() {
        let error = normalize_absolute_path(Path::new(""))
            .expect_err("empty paths should be rejected before normalization");

        assert!(error.to_string().contains("path"));
        assert!(error.to_string().contains("empty"));
    }

    #[test]
    fn point_for_offset_uses_tree_sitter_byte_columns() {
        let source = "é\nx";

        assert_eq!(
            point_for_offset(source, "é".len()).unwrap(),
            Point { row: 0, column: 2 }
        );
        assert_eq!(
            point_for_offset(source, "é\n".len()).unwrap(),
            Point { row: 1, column: 0 }
        );
    }

    #[test]
    fn offset_for_position_uses_tree_sitter_byte_columns() {
        let source = "é\nx";

        assert_eq!(
            offset_for_position(source, &Position { row: 0, column: 2 }).unwrap(),
            "é".len()
        );
        assert_eq!(
            offset_for_position(source, &Position { row: 1, column: 1 }).unwrap(),
            source.len()
        );
    }

    #[test]
    fn offset_for_position_rejects_non_boundary_byte_columns() {
        let source = "é\nx";

        let error = offset_for_position(source, &Position { row: 0, column: 1 })
            .expect_err("positions inside a UTF-8 character should be rejected");

        assert!(
            error
                .to_string()
                .contains("does not align to a UTF-8 character boundary")
        );
    }
}

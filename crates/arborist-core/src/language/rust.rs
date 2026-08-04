use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::Result;
use tree_sitter::Node;

use super::{node_text, normalize_absolute_path};

pub(crate) fn rust_local_module_dependency_paths(
    path: &Path,
    root: Node<'_>,
    source: &str,
) -> Result<BTreeSet<PathBuf>> {
    let mut dependencies = BTreeSet::new();
    collect_out_of_line_module_dependencies(path, root, source, &mut dependencies)?;
    Ok(dependencies)
}

fn collect_out_of_line_module_dependencies(
    path: &Path,
    node: Node<'_>,
    source: &str,
    dependencies: &mut BTreeSet<PathBuf>,
) -> Result<()> {
    if node.kind() == "mod_item"
        && node.child_by_field_name("body").is_none()
        && !has_path_semantics(node, source)?
        && let Some(name) = node
            .child_by_field_name("name")
            .map(|name| node_text(name, source))
            .transpose()?
            .and_then(normalize_rust_module_name)
    {
        let base = rust_module_directory(path, node, source)?;
        if let Some(module_path) = unique_rust_module_file(&base.join(name)) {
            dependencies.insert(module_path);
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_out_of_line_module_dependencies(path, child, source, dependencies)?;
    }
    Ok(())
}

fn has_path_semantics(node: Node<'_>, source: &str) -> Result<bool> {
    let mut sibling = node.prev_named_sibling();
    while let Some(attribute_item) = sibling {
        if attribute_item.kind() != "attribute_item" {
            break;
        }
        if is_path_semantic_attribute(attribute_item, source)? {
            return Ok(true);
        }
        sibling = attribute_item.prev_named_sibling();
    }
    Ok(false)
}

fn is_path_semantic_attribute(attribute_item: Node<'_>, source: &str) -> Result<bool> {
    let Some(attribute) = attribute_item.named_child(0) else {
        return Ok(false);
    };
    let Some(name) = attribute
        .named_child(0)
        .map(|name| node_text(name, source))
        .transpose()?
    else {
        return Ok(false);
    };

    match name.trim() {
        "path" => Ok(true),
        "cfg_attr" => Ok(attribute
            .child_by_field_name("arguments")
            .map(|arguments| cfg_attr_has_path_assignment(arguments, source))
            .transpose()?
            .unwrap_or(false)),
        _ => Ok(false),
    }
}

fn cfg_attr_has_path_assignment(arguments: Node<'_>, source: &str) -> Result<bool> {
    let arguments = node_text(arguments, source)?.trim();
    let Some(arguments) = arguments
        .strip_prefix('(')
        .and_then(|arguments| arguments.strip_suffix(')'))
    else {
        return Ok(false);
    };

    Ok(cfg_attr_arguments_have_path_semantics(arguments))
}

fn cfg_attr_arguments_have_path_semantics(arguments: &str) -> bool {
    split_top_level_attribute_arguments(arguments)
        .into_iter()
        .skip(1)
        .any(attribute_argument_has_path_semantics)
}

fn attribute_argument_has_path_semantics(argument: &str) -> bool {
    let argument = skip_rust_trivia(argument);
    if is_path_assignment(argument) {
        return true;
    }

    let Some(arguments) = argument.strip_prefix("cfg_attr") else {
        return false;
    };
    let arguments = skip_rust_trivia(arguments);
    let Some(arguments) = arguments
        .strip_prefix('(')
        .and_then(|arguments| arguments.strip_suffix(')'))
    else {
        return false;
    };

    cfg_attr_arguments_have_path_semantics(arguments)
}

fn split_top_level_attribute_arguments(arguments: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut nesting: usize = 0;
    let mut quote = None;
    let mut escaped = false;

    for (index, character) in arguments.char_indices() {
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == active_quote {
                quote = None;
            }
            continue;
        }

        match character {
            '"' | '\'' => quote = Some(character),
            '(' | '[' | '{' => nesting += 1,
            ')' | ']' | '}' => nesting = nesting.saturating_sub(1),
            ',' if nesting == 0 => {
                parts.push(&arguments[start..index]);
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(&arguments[start..]);
    parts
}

fn is_path_assignment(argument: &str) -> bool {
    let argument = skip_rust_trivia(argument);
    let Some(rest) = argument.strip_prefix("path") else {
        return false;
    };
    skip_rust_trivia(rest).starts_with('=')
}

fn skip_rust_trivia(mut source: &str) -> &str {
    loop {
        source = source.trim_start();
        if let Some(rest) = source.strip_prefix("//") {
            source = rest.split_once('\n').map_or("", |(_, rest)| rest);
            continue;
        }
        let Some(mut rest) = source.strip_prefix("/*") else {
            return source;
        };
        let mut depth = 1;
        while depth > 0 {
            if let Some(after_open) = rest.strip_prefix("/*") {
                depth += 1;
                rest = after_open;
            } else if let Some(after_close) = rest.strip_prefix("*/") {
                depth -= 1;
                rest = after_close;
            } else if let Some(character) = rest.chars().next() {
                rest = &rest[character.len_utf8()..];
            } else {
                return "";
            }
        }
        source = rest;
    }
}

fn rust_module_directory(path: &Path, node: Node<'_>, source: &str) -> Result<PathBuf> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_stem = path.file_stem().and_then(|stem| stem.to_str());
    let mut directory = match file_stem {
        Some("lib" | "main" | "mod") | None => parent.to_path_buf(),
        Some(stem) => parent.join(stem),
    };

    let mut inline_modules = Vec::new();
    let mut current = node.parent();
    while let Some(candidate) = current {
        if candidate.kind() == "mod_item"
            && candidate.child_by_field_name("body").is_some()
            && let Some(name) = candidate
                .child_by_field_name("name")
                .map(|name| node_text(name, source))
                .transpose()?
                .and_then(normalize_rust_module_name)
        {
            inline_modules.push(name);
        }
        current = candidate.parent();
    }
    for module in inline_modules.iter().rev() {
        directory.push(module);
    }
    Ok(directory)
}

fn unique_rust_module_file(base: &Path) -> Option<PathBuf> {
    let file_candidate = base.with_extension("rs");
    let directory_candidate = base.join("mod.rs");
    let path = match (file_candidate.is_file(), directory_candidate.is_file()) {
        (true, false) => file_candidate,
        (false, true) => directory_candidate,
        _ => return None,
    };
    normalize_absolute_path(&path).ok()
}

fn normalize_rust_module_name(value: &str) -> Option<String> {
    let value = value.trim();
    let value = value.strip_prefix("r#").unwrap_or(value);
    (!value.is_empty() && value != "." && value != ".." && !value.contains(['/', '\\', '\0']))
        .then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::rust_local_module_dependency_paths;
    use crate::language::{normalize_absolute_path, parse_document};

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temporary_dir() -> std::path::PathBuf {
        let suffix = format!(
            "{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let dir = std::env::temp_dir().join(format!("arborist-rust-language-{suffix}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn resolves_unambiguous_out_of_line_modules() {
        let workspace = temporary_dir();
        let src = workspace.join("src");
        fs::create_dir_all(src.join("nested")).unwrap();
        let root_path = src.join("lib.rs");
        let helper_path = src.join("helper.rs");
        let nested_path = src.join("nested").join("mod.rs");
        for path in [&helper_path, &nested_path] {
            fs::write(path, "pub fn item() {}\n").unwrap();
        }
        fs::write(
            &root_path,
            "mod helper;\nmod nested;\nuse crate::helper::item;\nuse crate::nested::item;\n",
        )
        .unwrap();

        let source = fs::read_to_string(&root_path).unwrap();
        let document = parse_document(&root_path, &source).unwrap();
        let dependencies =
            rust_local_module_dependency_paths(&root_path, document.tree.root_node(), &source)
                .unwrap();

        assert_eq!(
            dependencies,
            [helper_path, nested_path]
                .into_iter()
                .map(|path| normalize_absolute_path(&path).unwrap())
                .collect()
        );
    }

    #[test]
    fn resolves_child_modules_relative_to_nonstandard_target_root_files() {
        let workspace = temporary_dir();
        let src = workspace.join("src");
        let binary_dir = src.join("bin");
        let binary_module_dir = binary_dir.join("tool");
        fs::create_dir_all(&binary_module_dir).unwrap();
        let target_root = binary_dir.join("tool.rs");
        let target_helper = binary_module_dir.join("helper.rs");
        fs::write(src.join("lib.rs"), "pub fn library() {}\n").unwrap();
        fs::write(
            src.join("helper.rs"),
            "pub fn unrelated_library_helper() {}\n",
        )
        .unwrap();
        fs::write(&target_root, "mod helper;\nuse crate::helper::item;\n").unwrap();
        fs::write(&target_helper, "pub fn item() {}\n").unwrap();

        let source = fs::read_to_string(&target_root).unwrap();
        let document = parse_document(&target_root, &source).unwrap();
        let dependencies =
            rust_local_module_dependency_paths(&target_root, document.tree.root_node(), &source)
                .unwrap();

        assert_eq!(
            dependencies,
            [normalize_absolute_path(&target_helper).unwrap()]
                .into_iter()
                .collect()
        );
    }
    #[test]
    fn resolves_out_of_line_modules_declared_inside_inline_modules() {
        let workspace = temporary_dir();
        let src = workspace.join("src");
        fs::create_dir_all(src.join("inline")).unwrap();
        let root_path = src.join("lib.rs");
        let child_path = src.join("inline").join("child.rs");
        fs::write(&root_path, "mod inline {\n    mod child;\n}\n").unwrap();
        fs::write(&child_path, "pub fn item() {}\n").unwrap();

        let source = fs::read_to_string(&root_path).unwrap();
        let document = parse_document(&root_path, &source).unwrap();
        let dependencies =
            rust_local_module_dependency_paths(&root_path, document.tree.root_node(), &source)
                .unwrap();

        assert_eq!(
            dependencies,
            [normalize_absolute_path(&child_path).unwrap()]
                .into_iter()
                .collect()
        );
    }

    #[test]
    fn skips_path_semantic_modules_instead_of_following_the_default_module_layout() {
        let workspace = temporary_dir();
        let src = workspace.join("src");
        fs::create_dir_all(&src).unwrap();
        let root_path = src.join("lib.rs");
        fs::write(
            &root_path,
            "#[path = \"custom.rs\"]\nmod generated;\nuse crate::generated::item;\n#[cfg_attr(feature = \"generated\", path = \"other.rs\")]\nmod conditional;\n",
        )
        .unwrap();
        fs::write(src.join("generated.rs"), "pub fn default() {}\n").unwrap();
        fs::write(src.join("custom.rs"), "pub fn custom() {}\n").unwrap();
        fs::write(src.join("conditional.rs"), "pub fn default() {}\n").unwrap();
        fs::write(src.join("other.rs"), "pub fn other() {}\n").unwrap();

        let source = fs::read_to_string(&root_path).unwrap();
        let document = parse_document(&root_path, &source).unwrap();
        let dependencies =
            rust_local_module_dependency_paths(&root_path, document.tree.root_node(), &source)
                .unwrap();

        assert!(dependencies.is_empty());
    }

    #[test]
    fn does_not_treat_a_same_named_file_as_a_dependency_of_an_inline_module() {
        let workspace = temporary_dir();
        let src = workspace.join("src");
        fs::create_dir_all(&src).unwrap();
        let root_path = src.join("lib.rs");
        fs::write(
            &root_path,
            "mod api { pub fn item() {} }\nuse crate::api::item;\n",
        )
        .unwrap();
        fs::write(src.join("api.rs"), "pub fn unrelated() {}\n").unwrap();

        let source = fs::read_to_string(&root_path).unwrap();
        let document = parse_document(&root_path, &source).unwrap();
        let dependencies =
            rust_local_module_dependency_paths(&root_path, document.tree.root_node(), &source)
                .unwrap();

        assert!(dependencies.is_empty());
    }

    #[test]
    fn rejects_ambiguous_module_file_layouts_instead_of_guessing() {
        let workspace = temporary_dir();
        let src = workspace.join("src");
        fs::create_dir_all(src.join("ambiguous")).unwrap();
        let root_path = src.join("lib.rs");
        fs::write(&root_path, "mod ambiguous;\n").unwrap();
        fs::write(src.join("ambiguous.rs"), "pub fn one() {}\n").unwrap();
        fs::write(src.join("ambiguous").join("mod.rs"), "pub fn two() {}\n").unwrap();

        let source = fs::read_to_string(&root_path).unwrap();
        let document = parse_document(&root_path, &source).unwrap();
        let dependencies =
            rust_local_module_dependency_paths(&root_path, document.tree.root_node(), &source)
                .unwrap();

        assert!(dependencies.is_empty());
    }

    #[test]
    fn retains_modules_with_non_path_attributes_that_mention_path() {
        let workspace = temporary_dir();
        let src = workspace.join("src");
        fs::create_dir_all(&src).unwrap();
        let root_path = src.join("lib.rs");
        let helper_path = src.join("helper.rs");
        let conditional_path = src.join("conditional.rs");
        fs::write(
            &root_path,
            "#[deprecated(note = \"legacy path\")]\nmod helper;\n#[cfg_attr(feature = \"fast-path\", deprecated(note = \"legacy path\"))]\nmod conditional;\n",
        )
        .unwrap();
        fs::write(&helper_path, "pub fn item() {}\n").unwrap();
        fs::write(&conditional_path, "pub fn conditional() {}\n").unwrap();

        let source = fs::read_to_string(&root_path).unwrap();
        let document = parse_document(&root_path, &source).unwrap();
        let dependencies =
            rust_local_module_dependency_paths(&root_path, document.tree.root_node(), &source)
                .unwrap();

        assert_eq!(
            dependencies,
            [helper_path, conditional_path]
                .into_iter()
                .map(|path| normalize_absolute_path(&path).unwrap())
                .collect()
        );
    }

    #[test]
    fn resolves_raw_and_unicode_module_names() {
        let workspace = temporary_dir();
        let src = workspace.join("src");
        fs::create_dir_all(src.join("async")).unwrap();
        let root_path = src.join("lib.rs");
        let raw_path = src.join("await.rs");
        let unicode_path = src.join("café.rs");
        let raw_child_path = src.join("async").join("child.rs");
        fs::write(
            &root_path,
            "mod r#await;\nmod café;\nmod r#async { mod child; }\n",
        )
        .unwrap();
        fs::write(&raw_path, "pub fn raw() {}\n").unwrap();
        fs::write(&unicode_path, "pub fn unicode() {}\n").unwrap();
        fs::write(&raw_child_path, "pub fn child() {}\n").unwrap();

        let source = fs::read_to_string(&root_path).unwrap();
        let document = parse_document(&root_path, &source).unwrap();
        let dependencies =
            rust_local_module_dependency_paths(&root_path, document.tree.root_node(), &source)
                .unwrap();

        assert_eq!(
            dependencies,
            [raw_path, unicode_path, raw_child_path]
                .into_iter()
                .map(|path| normalize_absolute_path(&path).unwrap())
                .collect()
        );
    }

    #[test]
    fn skips_nested_cfg_attr_path_modules() {
        let workspace = temporary_dir();
        let src = workspace.join("src");
        fs::create_dir_all(src.join("platform")).unwrap();
        let root_path = src.join("lib.rs");
        fs::write(
            &root_path,
            "#[cfg_attr(unix, cfg_attr(feature = \"alternate-layout\", path = \"platform/alternate.rs\"))]\nmod platform;\n",
        )
        .unwrap();
        fs::write(src.join("platform.rs"), "pub fn default() {}\n").unwrap();
        fs::write(
            src.join("platform").join("alternate.rs"),
            "pub fn alternate() {}\n",
        )
        .unwrap();

        let source = fs::read_to_string(&root_path).unwrap();
        let document = parse_document(&root_path, &source).unwrap();
        let dependencies =
            rust_local_module_dependency_paths(&root_path, document.tree.root_node(), &source)
                .unwrap();

        assert!(dependencies.is_empty());
    }

    #[test]
    fn skips_commented_cfg_attr_path_syntax() {
        let workspace = temporary_dir();
        let src = workspace.join("src");
        fs::create_dir_all(src.join("platform")).unwrap();
        let root_path = src.join("lib.rs");
        fs::write(
            &root_path,
            "#[cfg_attr(unix, path /* select alternate source */ = \"platform/alternate.rs\")]\nmod platform;\n#[cfg_attr /* nested override */ (unix, cfg_attr(feature = \"alternate\", path = \"platform/alternate.rs\"))]\nmod nested_platform;\n",
        )
        .unwrap();
        fs::write(src.join("platform.rs"), "pub fn default() {}\n").unwrap();
        fs::write(src.join("nested_platform.rs"), "pub fn default() {}\n").unwrap();
        fs::write(
            src.join("platform").join("alternate.rs"),
            "pub fn alternate() {}\n",
        )
        .unwrap();

        let source = fs::read_to_string(&root_path).unwrap();
        let document = parse_document(&root_path, &source).unwrap();
        let dependencies =
            rust_local_module_dependency_paths(&root_path, document.tree.root_node(), &source)
                .unwrap();

        assert!(dependencies.is_empty());
    }

    #[test]
    fn malformed_rust_source_does_not_make_dependency_extraction_fail() {
        let path = Path::new("broken.rs");
        let source = "mod broken(\nuse crate::also_broken::{value};\n";
        let document = parse_document(path, source).unwrap();
        let dependencies =
            rust_local_module_dependency_paths(path, document.tree.root_node(), source).unwrap();
        assert!(dependencies.is_empty());
    }
}

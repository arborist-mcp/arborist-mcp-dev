use super::path::*;
use super::scope::*;
use super::types::*;
use crate::language::normalize_path;
use crate::workspace_scan::WorkspaceScanDeadline;
use anyhow::Result;
use std::path::Path;
use tree_sitter::Node;

pub(in super::super) fn collect_python_local_bindings(
    current_path: &Path,
    node: Node<'_>,
    source: &str,
) -> Result<Vec<PythonAccessibleSymbol>> {
    collect_python_local_bindings_with_deadline(current_path, node, source, None)
}

pub(in super::super) fn collect_python_local_bindings_with_deadline(
    current_path: &Path,
    node: Node<'_>,
    source: &str,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Vec<PythonAccessibleSymbol>> {
    if let Some(deadline) = deadline {
        deadline.check("collecting Python local bindings")?;
    }
    let normalized_path = normalize_path(current_path);
    let scope_path = python_binding_scope_path(node, source)?;
    let origin_type = if node.kind() == "module" {
        "module_scope"
    } else {
        "local_scope"
    };

    let mut symbols = Vec::new();
    if node.kind() == "lambda" {
        if node.child_by_field_name("body").is_none() {
            return Ok(Vec::new());
        }
        if let Some(deadline) = deadline {
            deadline.check("collecting Python local bindings")?;
        }
        collect_python_scope_symbols(node, source, &normalized_path, 0, &mut symbols)?;
        return Ok(symbols);
    }

    let body_node = if node.kind() == "module" {
        node
    } else if let Some(body) = node.child_by_field_name("body") {
        body
    } else {
        return Ok(Vec::new());
    };

    let class_visibility =
        (node.kind() == "class_definition").then_some((node.start_byte(), node.end_byte()));
    let mut cursor = body_node.walk();
    for statement in body_node.named_children(&mut cursor) {
        if let Some(deadline) = deadline {
            deadline.check("collecting Python local bindings")?;
        }
        let mut statement_symbols = Vec::new();
        collect_python_statement_symbols(
            statement,
            source,
            &normalized_path,
            scope_path.as_deref(),
            origin_type,
            0,
            &mut statement_symbols,
        )?;
        if let Some(class_range) = class_visibility {
            for symbol in &mut statement_symbols {
                if matches!(symbol.visibility, PythonSymbolVisibility::Always) {
                    symbol.visibility = PythonSymbolVisibility::ClassBodyLocal { class_range };
                }
            }
        }
        symbols.extend(statement_symbols);
    }

    let external_bindings = match deadline {
        Some(deadline) => {
            collect_python_external_binding_names_with_deadline(body_node, source, Some(deadline))?
        }
        None => collect_python_external_binding_names(body_node, source)?,
    };
    if !external_bindings.is_empty() {
        symbols.retain(|symbol| !external_bindings.contains(&symbol.name));
    }
    Ok(symbols)
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::{Duration, Instant};

    use super::collect_python_local_bindings_with_deadline;
    use crate::language::parse_document;
    use crate::workspace_scan::WorkspaceScanDeadline;

    #[test]
    fn local_binding_collection_rejects_expired_deadline() {
        let source = "def sample(value):\n    result = value\n    return result\n";
        let path = Path::new("sample.py");
        let document = parse_document(path, source).expect("source should parse");
        let deadline = WorkspaceScanDeadline {
            deadline: Some(Instant::now() - Duration::from_millis(1)),
            timeout_ms: Some(1),
        };

        let error = collect_python_local_bindings_with_deadline(
            path,
            document.tree.root_node(),
            source,
            Some(&deadline),
        )
        .expect_err("expired local binding collection should fail");
        assert!(
            error
                .to_string()
                .contains("workspace scan timeout exceeded")
        );
    }
}

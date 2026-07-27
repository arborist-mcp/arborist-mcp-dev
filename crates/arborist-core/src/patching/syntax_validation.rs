use tree_sitter::Node;

use crate::deadline::DeadlineCheck;
use crate::language::{position_from, visit_tree, visit_tree_with_deadline};
use crate::model::{Position, ValidationIssue};

pub(crate) fn collect_syntax_errors(root: Node<'_>, source: &str) -> Vec<ValidationIssue> {
    collect_syntax_errors_with_deadline(root, source, None)
        .expect("deadline-free syntax validation cannot fail")
}

pub(crate) fn collect_syntax_errors_with_deadline(
    root: Node<'_>,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> anyhow::Result<Vec<ValidationIssue>> {
    let mut issues = Vec::new();
    let mut callback = |node: Node<'_>| {
        if node.is_error() || node.is_missing() {
            let kind = if node.is_missing() {
                "missing"
            } else {
                "error"
            };
            issues.push(ValidationIssue {
                kind: kind.to_string(),
                message: format!("Tree-sitter reported a {kind} node near `{}`", node.kind()),
                start_byte: node.start_byte(),
                end_byte: node.end_byte(),
                start_point: position_from(node.start_position()),
                end_point: position_from(node.end_position()),
            });
        } else if node.kind() == "ERROR" {
            issues.push(ValidationIssue {
                kind: "error".to_string(),
                message: format!(
                    "Tree-sitter produced an ERROR node near `{}`",
                    node.utf8_text(source.as_bytes()).unwrap_or(node.kind())
                ),
                start_byte: node.start_byte(),
                end_byte: node.end_byte(),
                start_point: position_from(node.start_position()),
                end_point: position_from(node.end_position()),
            });
        }
    };

    match deadline {
        Some(deadline) => visit_tree_with_deadline(root, &mut callback, deadline)?,
        None => visit_tree(root, &mut callback),
    }
    if root.kind() == "module" {
        issues.extend(collect_python_indentation_issues(source, deadline)?);
    }
    Ok(issues)
}

fn collect_python_indentation_issues(
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> anyhow::Result<Vec<ValidationIssue>> {
    let mut issues = Vec::new();
    let mut pending_block: Option<(usize, usize, usize)> = None;
    let mut byte_start = 0usize;

    for (row, line) in source.split_inclusive('\n').enumerate() {
        if let Some(deadline) = deadline {
            deadline.check("validating Python indentation")?;
        }
        let content = line.trim_end_matches(['\r', '\n']);
        let trimmed = content.trim();
        let indent = leading_indent_len(content);

        if !trimmed.is_empty() && !trimmed.starts_with('#') {
            if let Some((header_indent, header_row, header_start)) = pending_block.take()
                && indent <= header_indent
            {
                issues.push(ValidationIssue {
                    kind: "indentation".to_string(),
                    message: format!(
                        "Python indentation appears invalid: expected an indented block after line {}",
                        header_row + 1
                    ),
                    start_byte: byte_start,
                    end_byte: byte_start + content.len(),
                    start_point: Position {
                        row,
                        column: 0,
                    },
                    end_point: Position {
                        row,
                        column: content.len(),
                    },
                });
                pending_block = Some((header_indent, header_row, header_start));
            }

            if trimmed.ends_with(':') {
                pending_block = Some((indent, row, byte_start));
            }
        }

        byte_start += line.len();
    }

    Ok(issues)
}

fn leading_indent_len(line: &str) -> usize {
    line.as_bytes()
        .iter()
        .take_while(|byte| **byte == b' ' || **byte == b'\t')
        .count()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::collect_syntax_errors_with_deadline;
    use crate::deadline::CooperativeDeadline;
    use crate::language::parse_document;

    #[test]
    fn syntax_validation_checks_expired_patch_preview_deadlines() {
        let source = "def sample():\n    return 1\n";
        let document = parse_document(Path::new("sample.py"), source).expect("source should parse");
        let deadline = CooperativeDeadline::expired_for_tests(1, "patch preview");

        let error =
            collect_syntax_errors_with_deadline(document.tree.root_node(), source, Some(&deadline))
                .expect_err("expired syntax validation should fail");

        assert!(
            error
                .to_string()
                .contains("patch preview timeout exceeded during walking syntax tree")
        );
    }
}

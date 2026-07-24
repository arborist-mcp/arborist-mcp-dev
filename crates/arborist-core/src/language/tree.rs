use anyhow::Result;
use tree_sitter::Node;

pub fn node_text<'a>(node: Node<'_>, source: &'a str) -> Result<&'a str> {
    Ok(node.utf8_text(source.as_bytes())?)
}

pub fn visit_tree(node: Node<'_>, callback: &mut impl FnMut(Node<'_>)) {
    callback(node);
    let child_count = node.child_count();
    for index in 0..child_count {
        if let Some(child) = node.child(index) {
            visit_tree(child, callback);
        }
    }
}

pub fn contains_kind(node: Node<'_>, wanted: &str) -> bool {
    if node.kind() == wanted {
        return true;
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if contains_kind(child, wanted) {
            return true;
        }
    }

    false
}

pub fn first_identifier(node: Node<'_>, source: &str) -> Result<Option<String>> {
    find_identifier(node, &["identifier", "field_identifier"], source)
}

pub fn last_type_identifier(node: Node<'_>, source: &str) -> Result<Option<String>> {
    let mut hits = Vec::new();
    collect_identifiers(node, &["type_identifier"], source, &mut hits)?;
    Ok(hits.pop())
}

pub fn find_identifier(node: Node<'_>, kinds: &[&str], source: &str) -> Result<Option<String>> {
    if kinds.contains(&node.kind()) {
        return Ok(Some(node_text(node, source)?.trim().to_string()));
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if let Some(found) = find_identifier(child, kinds, source)? {
            return Ok(Some(found));
        }
    }

    Ok(None)
}

pub fn collect_identifiers(
    node: Node<'_>,
    kinds: &[&str],
    source: &str,
    hits: &mut Vec<String>,
) -> Result<()> {
    if kinds.contains(&node.kind()) {
        hits.push(node_text(node, source)?.trim().to_string());
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_identifiers(child, kinds, source, hits)?;
    }

    Ok(())
}

pub fn is_field_node(parent: Node<'_>, field_name: &str, node: Node<'_>) -> bool {
    parent
        .child_by_field_name(field_name)
        .is_some_and(|candidate| candidate.id() == node.id())
}

pub fn contains_node(container: Node<'_>, node: Node<'_>) -> bool {
    container.start_byte() <= node.start_byte() && container.end_byte() >= node.end_byte()
}

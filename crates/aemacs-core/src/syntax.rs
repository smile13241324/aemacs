use anyhow::{anyhow, Result};
use std::fs;
use std::path::Path;
use tree_sitter::{Parser, Query, QueryCursor, StreamingIterator};

/// Extracts the source code of a specific symbol (struct, enum, impl, fn) from a Rust file.
pub fn extract_symbol(path: &Path, symbol_name: &str) -> Result<String> {
    let source_code = fs::read_to_string(path)?;
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_rust::LANGUAGE.into())?;

    let tree = parser
        .parse(&source_code, None)
        .ok_or_else(|| anyhow!("Failed to parse file: {:?}", path))?;

    // Tree-sitter query to find structs, enums, impls, and functions by name.
    // Captures the whole node.
    let query_str = r#"
        (struct_item name: (type_identifier) @name) @item
        (enum_item name: (type_identifier) @name) @item
        (function_item name: (identifier) @name) @item
        (impl_item type: (type_identifier) @name) @item
        (trait_item name: (type_identifier) @name) @item
    "#;

    let query = Query::new(&tree_sitter_rust::LANGUAGE.into(), query_str)?;
    let mut cursor = QueryCursor::new();
    let mut captures = cursor.captures(&query, tree.root_node(), source_code.as_bytes());

    while let Some((m, capture_index)) = captures.next() {
        let capture = m.captures[*capture_index];
        let capture_name = query.capture_names()[capture.index as usize];

        if capture_name == "name" {
            if let Ok(name) = capture.node.utf8_text(source_code.as_bytes()) {
                if name == symbol_name {
                    // Once we find the name, the item is the node with the "item" capture index in the same match
                    for c in m.captures {
                        if query.capture_names()[c.index as usize] == "item" {
                            return Ok(c.node.utf8_text(source_code.as_bytes())?.to_string());
                        }
                    }
                }
            }
        }
    }

    Err(anyhow!(
        "Symbol '{}' not found in {:?}",
        symbol_name,
        path
    ))
}

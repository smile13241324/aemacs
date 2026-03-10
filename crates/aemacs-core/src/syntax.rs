use anyhow::{Result, anyhow};
use std::fs;
use std::path::Path;
use tree_sitter::{Parser, Query, QueryCursor, StreamingIterator};

/// Supported languages for AST extraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportedLanguage {
    Rust,
    Python,
    Go,
    Haskell,
    C,
    Cpp,
    JavaScript,
}

impl SupportedLanguage {
    /// Detects the language from a file extension.
    pub fn from_path(path: &Path) -> Result<Self> {
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow!("No file extension found for {:?}", path))?;

        match ext.to_lowercase().as_str() {
            "rs" => Ok(Self::Rust),
            "py" => Ok(Self::Python),
            "go" => Ok(Self::Go),
            "hs" => Ok(Self::Haskell),
            "c" | "h" => Ok(Self::C),
            "cpp" | "hpp" | "cc" | "cxx" => Ok(Self::Cpp),
            "js" | "mjs" | "cjs" => Ok(Self::JavaScript),
            _ => Err(anyhow!("Unsupported language extension: .{}", ext)),
        }
    }

    /// Returns the tree-sitter language and the query string for the specific language.
    pub fn get_config(&self) -> (tree_sitter::Language, &'static str) {
        match self {
            Self::Rust => (
                tree_sitter_rust::LANGUAGE.into(),
                r#"
                (struct_item name: (type_identifier) @name) @item
                (enum_item name: (type_identifier) @name) @item
                (function_item name: (identifier) @name) @item
                (impl_item type: (type_identifier) @name) @item
                (trait_item name: (type_identifier) @name) @item
                "#,
            ),
            Self::Python => (
                tree_sitter_python::LANGUAGE.into(),
                r#"
                (function_definition name: (identifier) @name) @item
                (class_definition name: (identifier) @name) @item
                "#,
            ),
            Self::Go => (
                tree_sitter_go::LANGUAGE.into(),
                r#"
                (function_declaration name: (identifier) @name) @item
                (method_declaration name: (field_identifier) @name) @item
                (type_declaration (type_spec name: (type_identifier) @name)) @item
                "#,
            ),
            Self::Haskell => (
                tree_sitter_haskell::LANGUAGE.into(),
                r#"
                (signature name: (variable) @name) @item
                (function name: (variable) @name) @item
                (data_type name: (_) @name) @item
                "#,
            ),
            Self::C => (
                tree_sitter_c::LANGUAGE.into(),
                r#"
                (struct_specifier name: (type_identifier) @name) @item
                (enum_specifier name: (type_identifier) @name) @item
                (function_definition declarator: (function_declarator declarator: (identifier) @name)) @item
                "#,
            ),
            Self::Cpp => (
                tree_sitter_cpp::LANGUAGE.into(),
                r#"
                (class_specifier name: (type_identifier) @name) @item
                (struct_specifier name: (type_identifier) @name) @item
                (function_definition declarator: (function_declarator declarator: (identifier) @name)) @item
                "#,
            ),
            Self::JavaScript => (
                tree_sitter_javascript::LANGUAGE.into(),
                r#"
                (function_declaration name: (identifier) @name) @item
                (class_declaration name: (identifier) @name) @item
                (method_definition name: (property_identifier) @name) @item
                (variable_declarator name: (identifier) @name) @item
                "#,
            ),
        }
    }
}

/// Extracts the source code of a specific symbol from a file using Tree-sitter.
/// Supports Rust, Python, Go, Haskell, C, C++, Clojure, and JavaScript.
pub fn extract_symbol(
    path: &Path,
    symbol_name: &str,
    language_override: Option<SupportedLanguage>,
) -> Result<String> {
    let lang = match language_override {
        Some(l) => l,
        None => SupportedLanguage::from_path(path)?,
    };
    let (ts_lang, query_str) = lang.get_config();

    let source_code = fs::read_to_string(path)?;
    let mut parser = Parser::new();
    parser.set_language(&ts_lang.clone().into())?;

    let tree = parser
        .parse(&source_code, None)
        .ok_or_else(|| anyhow!("Failed to parse file: {:?}", path))?;

    let query = Query::new(&ts_lang.into(), query_str)?;
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
        "Symbol '{}' not found in {:?} (Detected Language: {:?})",
        symbol_name,
        path,
        lang
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_extract_python_symbol() -> Result<()> {
        let mut file = NamedTempFile::new().unwrap();
        let code = r#"
class Forge:
    def strike(self):
        print("Iron")

def global_melt():
    pass
"#;
        file.write_all(code.as_bytes()).unwrap();
        let path = file.path().with_extension("py");
        std::fs::rename(file.path(), &path).unwrap();

        let result = extract_symbol(&path, "Forge", None)?;
        assert!(result.contains("class Forge:"));
        assert!(result.contains("def strike"));

        let result = extract_symbol(&path, "global_melt", None)?;
        assert!(result.contains("def global_melt():"));

        Ok(())
    }

    #[test]
    fn test_extract_go_symbol() -> Result<()> {
        let mut file = NamedTempFile::new().unwrap();
        let code = r#"
package main

type Architect struct {
    Name string
}

func (a *Architect) Build() {
    println("Solid")
}

func MainCore() {
}
"#;
        file.write_all(code.as_bytes()).unwrap();
        let path = file.path().with_extension("go");
        std::fs::rename(file.path(), &path).unwrap();

        let result = extract_symbol(&path, "Architect", None)?;
        assert!(result.contains("type Architect struct"));

        let result = extract_symbol(&path, "Build", None)?;
        assert!(result.contains("func (a *Architect) Build()"));

        let result = extract_symbol(&path, "MainCore", None)?;
        assert!(result.contains("func MainCore()"));

        Ok(())
    }

    #[test]
    fn test_extract_haskell_symbol() -> Result<()> {
        let mut file = NamedTempFile::new().unwrap();
        let code = r#"
module Main where

-- The Core Truth
pureLogic :: Int -> Int
pureLogic x = x + 1

data Mind = Sovereign | Bound
"#;
        file.write_all(code.as_bytes()).unwrap();
        let path = file.path().with_extension("hs");
        std::fs::rename(file.path(), &path).unwrap();

        // Testing signature extraction
        let result = extract_symbol(&path, "pureLogic", None)?;
        assert!(
            result.contains("pureLogic :: Int -> Int") || result.contains("pureLogic x = x + 1")
        );

        // Testing data type extraction
        let result = extract_symbol(&path, "Mind", None)?;
        assert!(result.contains("data Mind"));

        Ok(())
    }

    #[test]
    fn test_extract_js_symbol() -> Result<()> {
        let mut file = NamedTempFile::new().unwrap();
        let code = r#"
class Visualizer {
    render() {
        console.log("Light");
    }
}

function processIntent(intent) {
    return true;
}

const handlePulse = (data) => {
    return data;
}
"#;
        file.write_all(code.as_bytes()).unwrap();
        let path = file.path().with_extension("js");
        std::fs::rename(file.path(), &path).unwrap();

        let result = extract_symbol(&path, "Visualizer", None)?;
        assert!(result.contains("class Visualizer"));

        let result = extract_symbol(&path, "processIntent", None)?;
        assert!(result.contains("function processIntent"));

        let result = extract_symbol(&path, "handlePulse", None)?;
        assert!(result.contains("handlePulse = (data) =>"));

        Ok(())
    }

    #[test]
    fn test_language_override() -> Result<()> {
        let mut file = NamedTempFile::new().unwrap();
        let code = "fn mystery() {}";
        // Extension is .tmp, but we tell it it's Rust
        file.write_all(code.as_bytes()).unwrap();

        let result = extract_symbol(file.path(), "mystery", Some(SupportedLanguage::Rust))?;
        assert!(result.contains("fn mystery()"));

        Ok(())
    }
}

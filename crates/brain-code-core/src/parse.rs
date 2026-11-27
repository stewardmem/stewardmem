//! The tree-sitter ingest (S3): symbols and occurrences with the alignment
//! law of §0.6 item 9 — every occurrence is aligned or NULL, never guessed
//! (pin 20). A reference aligns to a definition only when its name matches
//! exactly one candidate in scope; for C0 the candidate set is the file's
//! own definitions (cross-file resolution is C2/C3 work and is not
//! pretended here). A file whose root still has errors is ingested for its
//! clean symbols AND counted in `files_with_errors`.

use tree_sitter::{Node, Parser, Tree};

use crate::error::CodeIndexError;
use crate::inventory::FileLang;

/// Grammar ABI floor (pin 15): every grammar must be inside the library's
/// compatibility range or the ingest refuses before the first file.
pub fn assert_grammar_abis_in_range() -> Result<(), CodeIndexError> {
    let min = tree_sitter::MIN_COMPATIBLE_LANGUAGE_VERSION;
    let max = tree_sitter::LANGUAGE_VERSION;
    for (name, abi) in [
        ("tree-sitter-rust", grammar_abi(FileLang::Rust)?),
        (
            "tree-sitter-typescript(.ts)",
            grammar_abi(FileLang::TypeScript)?,
        ),
        ("tree-sitter-typescript(.tsx)", grammar_abi(FileLang::Tsx)?),
    ] {
        if abi < min || abi > max {
            return Err(CodeIndexError::Refusal(format!(
                "grammar {name} ABI {abi} is outside the library range {min}..={max} — refusing (pin 15)"
            )));
        }
    }
    Ok(())
}

fn language_for(lang: FileLang) -> Result<tree_sitter::Language, CodeIndexError> {
    let language: tree_sitter::Language = match lang {
        FileLang::Rust => tree_sitter_rust::LANGUAGE.into(),
        FileLang::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        FileLang::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
    };
    Ok(language)
}

fn grammar_abi(lang: FileLang) -> Result<usize, CodeIndexError> {
    Ok(language_for(lang)?.abi_version())
}

/// Rust definition kinds → the schema's `kind` vocabulary.
const RUST_DEFS: &[(&str, &str)] = &[
    ("function_item", "fn"),
    ("struct_item", "struct"),
    ("enum_item", "enum"),
    ("union_item", "union"),
    ("trait_item", "trait"),
    ("impl_item", "impl"),
    ("type_item", "type"),
    ("const_item", "const"),
    ("static_item", "static"),
    ("mod_item", "mod"),
];

/// TypeScript definition kinds → the schema's `kind` vocabulary.
const TS_DEFS: &[(&str, &str)] = &[
    ("function_declaration", "fn"),
    ("generator_function_declaration", "fn"),
    ("class_declaration", "class"),
    ("abstract_class_declaration", "class"),
    ("interface_declaration", "interface"),
    ("type_alias_declaration", "type_alias"),
    ("enum_declaration", "enum"),
    ("module", "module"),
    ("internal_module", "module"),
    ("method_definition", "method"),
];

/// Kinds whose children are qualified THROUGH this node (its own name/type
/// prefixes the children's names).
const RUST_SCOPES: &[&str] = &["mod_item", "impl_item"];
const TS_SCOPES: &[&str] = &[
    "class_declaration",
    "abstract_class_declaration",
    "module",
    "internal_module",
];

/// Identifier-bearing kinds that are candidate references.
const RUST_REFS: &[&str] = &[
    "identifier",
    "type_identifier",
    "field_identifier",
    "property_identifier",
];
const TS_REFS: &[&str] = &[
    "identifier",
    "property_identifier",
    "shorthand_property_identifier",
    "type_identifier",
];

#[derive(Debug)]
pub struct SymbolDraft {
    pub kind: &'static str,
    pub name: String,
    pub qualified_name: String,
    pub signature: Option<String>,
    pub start_line: usize,
    pub end_line: usize,
    pub start_byte: usize,
    pub end_byte: usize,
    pub visibility: Option<&'static str>,
    pub parent: Option<usize>,
    /// Where the definition's NAME sits — the `def` occurrence's position.
    pub def_start_line: usize,
    pub def_start_col: usize,
    pub def_end_line: usize,
    pub def_end_col: usize,
}

#[derive(Debug)]
pub struct RefDraft {
    pub name: String,
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
    /// Index into `symbols` when exactly one same-file candidate matched —
    /// `None` means "not aligned", never "aligned to nothing" (§0.6 item 9).
    pub symbol_index: Option<usize>,
}

#[derive(Debug)]
pub struct ParsedFile {
    pub symbols: Vec<SymbolDraft>,
    pub refs: Vec<RefDraft>,
    pub has_error: bool,
    /// Byte ranges of TOP-LEVEL nodes that belong to no definition — the
    /// windowed-chunk material (§2 S4). Empty when every top-level node is a
    /// definition.
    pub nondef_top_ranges: Vec<(usize, usize)>,
}

fn rust_visibility(node: Node, src: &str) -> Option<&'static str> {
    let text = &src[node.start_byte()..src.len().min(node.start_byte() + 12)];
    if text.starts_with("pub(") {
        Some("crate")
    } else if text.starts_with("pub") {
        Some("public")
    } else {
        Some("private")
    }
}

fn first_line(text: &str) -> String {
    text.lines().next().unwrap_or("").trim().to_string()
}

/// The signature slice: everything before the body for function-like nodes,
/// the first line otherwise.
fn signature_of(node: Node, src: &str) -> Option<String> {
    if let Some(body) = node.child_by_field_name("body")
        && body.start_byte() > node.start_byte()
    {
        return Some(
            src[node.start_byte()..body.start_byte()]
                .trim_end()
                .to_string(),
        );
    }
    let text = &src[node.start_byte()..node.end_byte()];
    let line = first_line(text);
    if line.is_empty() { None } else { Some(line) }
}

struct Scope {
    prefix: String,
    node_end_byte: usize,
    parent_symbol: Option<usize>,
}

pub fn parse(lang: FileLang, src: &str) -> Result<ParsedFile, CodeIndexError> {
    let language = language_for(lang)?;
    let mut parser = Parser::new();
    parser.set_language(&language).map_err(|e| {
        CodeIndexError::Refusal(format!("grammar for {lang:?} refused to load: {e}"))
    })?;
    let tree: Tree = parser.parse(src.as_bytes(), None).ok_or_else(|| {
        CodeIndexError::Refusal(format!("tree-sitter returned no tree for a {lang:?} file"))
    })?;
    let has_error = tree.root_node().has_error();

    let is_rust = lang == FileLang::Rust;
    let defs: &[(&str, &str)] = if is_rust { RUST_DEFS } else { TS_DEFS };
    let scopes: &[&str] = if is_rust { RUST_SCOPES } else { TS_SCOPES };
    let ref_kinds: &[&str] = if is_rust { RUST_REFS } else { TS_REFS };
    let joiner = if is_rust { "::" } else { "." };

    let mut symbols: Vec<SymbolDraft> = Vec::new();
    let mut def_name_nodes: Vec<(Node, usize)> = Vec::new();
    let mut stack: Vec<Scope> = Vec::new();

    // Top-level stretches that belong to no definition — the windowed-chunk
    // material (§2 S4).
    let mut nondef_top_ranges: Vec<(usize, usize)> = Vec::new();
    {
        let root = tree.root_node();
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            let kind = child.kind();
            if defs.iter().any(|(k, _)| *k == kind) {
                continue;
            }
            let text = &src[child.start_byte()..child.end_byte()];
            if text.trim().is_empty() {
                continue;
            }
            nondef_top_ranges.push((child.start_byte(), child.end_byte()));
        }
    }

    // Pass 1: definitions and the qualification stack.
    walk(tree.root_node(), &mut |node| {
        while stack
            .last()
            .is_some_and(|s| node.end_byte() > s.node_end_byte)
        {
            stack.pop();
        }
        let kind = node.kind();
        let scope_parent = stack.last().and_then(|s| s.parent_symbol);
        if let Some(&(_, symbol_kind)) = defs.iter().find(|(k, _)| *k == kind) {
            let name_node = node.child_by_field_name("name");
            let name = if kind == "impl_item" {
                // `impl Type` / `impl Trait for Type` — the type is the name.
                node.child_by_field_name("type")
                    .map(|t| src[t.start_byte()..t.end_byte()].to_string())
            } else {
                name_node.map(|n| src[n.start_byte()..n.end_byte()].to_string())
            };
            if let Some(name) = name {
                let prefix = stack.last().map(|s| s.prefix.as_str()).unwrap_or("");
                let qualified_name = if prefix.is_empty() {
                    name.clone()
                } else {
                    format!("{prefix}{joiner}{name}")
                };
                let signature = signature_of(node, src);
                let visibility = if is_rust {
                    rust_visibility(node, src)
                } else {
                    None
                };
                let parent = scope_parent;
                let index = symbols.len();
                let (ds_line, ds_col, de_line, de_col) = name_node
                    .map(|n| {
                        (
                            n.start_position().row + 1,
                            n.start_position().column,
                            n.end_position().row + 1,
                            n.end_position().column,
                        )
                    })
                    .unwrap_or((
                        node.start_position().row + 1,
                        node.start_position().column,
                        node.end_position().row + 1,
                        node.end_position().column,
                    ));
                symbols.push(SymbolDraft {
                    kind: symbol_kind,
                    name,
                    qualified_name,
                    signature,
                    start_line: node.start_position().row + 1,
                    end_line: node.end_position().row + 1,
                    start_byte: node.start_byte(),
                    end_byte: node.end_byte(),
                    visibility,
                    parent,
                    def_start_line: ds_line,
                    def_start_col: ds_col,
                    def_end_line: de_line,
                    def_end_col: de_col,
                });
                if let Some(name_node) = name_node {
                    def_name_nodes.push((name_node, index));
                }
                // An impl scopes its children by its TYPE, not "impl …".
                let scope_prefix = if kind == "impl_item" {
                    node.child_by_field_name("type")
                        .map(|t| src[t.start_byte()..t.end_byte()].to_string())
                } else {
                    Some(symbols[index].name.clone())
                };
                if scopes.contains(&kind)
                    && let Some(prefix) = scope_prefix
                {
                    stack.push(Scope {
                        prefix,
                        node_end_byte: node.end_byte(),
                        parent_symbol: Some(index),
                    });
                }
                // The definition's own subtree is visited by the generic
                // walk below; only its scope bookkeeping happens here.
            }
        }
    });

    // Pass 2: references, aligned when exactly one same-file candidate.
    let mut name_to_symbols: std::collections::BTreeMap<&str, Vec<usize>> =
        std::collections::BTreeMap::new();
    for (index, symbol) in symbols.iter().enumerate() {
        name_to_symbols
            .entry(symbol.name.as_str())
            .or_default()
            .push(index);
    }
    let def_name_ids: std::collections::HashSet<usize> =
        def_name_nodes.iter().map(|(node, _)| node.id()).collect();
    let mut refs: Vec<RefDraft> = Vec::new();
    walk(tree.root_node(), &mut |node| {
        if def_name_ids.contains(&node.id()) {
            return;
        }
        if ref_kinds.contains(&node.kind()) {
            let name = &src[node.start_byte()..node.end_byte()];
            let candidates = name_to_symbols
                .get(name)
                .map(|v| v.as_slice())
                .unwrap_or(&[]);
            let symbol_index = if candidates.len() == 1 {
                Some(candidates[0])
            } else {
                None
            };
            refs.push(RefDraft {
                name: name.to_string(),
                start_line: node.start_position().row + 1,
                start_col: node.start_position().column,
                end_line: node.end_position().row + 1,
                end_col: node.end_position().column,
                symbol_index,
            });
        }
    });

    Ok(ParsedFile {
        symbols,
        refs,
        has_error,
        nondef_top_ranges,
    })
}

/// A pre-order walk that never relies on a definition's own subtree being
/// entered through its name: every child is visited. The explicit lifetime
/// is load-bearing: eliding it makes walk higher-ranked and the parse fn's
/// captured `def_name_nodes` can no longer accept the walked node (E0521).
fn walk<'a>(node: Node<'a>, visit: &mut dyn FnMut(Node<'a>)) {
    visit(node);
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk(child, visit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grammar_abis_are_inside_the_library_range() {
        assert_grammar_abis_in_range().unwrap();
    }

    #[test]
    fn rust_symbols_carry_scope_qualified_names_and_signatures() {
        let src = "pub fn alpha() -> u8 {\n    beta()\n}\n\nstruct Widget {\n    id: u8,\n}\n\nimpl Widget {\n    pub fn refresh(&self) {}\n}\n\nmod inner {\n    pub fn deep() {}\n}\n";
        let parsed = parse(FileLang::Rust, src).unwrap();
        let names: Vec<&str> = parsed
            .symbols
            .iter()
            .map(|s| s.qualified_name.as_str())
            .collect();
        assert!(names.contains(&"alpha"), "{names:?}");
        assert!(names.contains(&"Widget"), "{names:?}");
        assert!(names.contains(&"Widget::refresh"), "{names:?}");
        assert!(names.contains(&"inner::deep"), "{names:?}");
        let alpha = parsed.symbols.iter().find(|s| s.name == "alpha").unwrap();
        assert_eq!(alpha.kind, "fn");
        assert_eq!(alpha.visibility, Some("public"));
        assert!(
            alpha
                .signature
                .as_deref()
                .unwrap_or("")
                .starts_with("pub fn alpha() -> u8")
        );
        // `beta()` is a reference to nothing — zero same-file candidates.
        let beta = parsed.refs.iter().find(|r| r.name == "beta").unwrap();
        assert!(
            beta.symbol_index.is_none(),
            "an unresolvable ref is NULL, never guessed"
        );
    }

    #[test]
    fn a_unique_same_file_reference_aligns_and_an_ambiguous_one_does_not() {
        let src = "fn helper() {}\n\nfn caller() {\n    let _ = helper();\n}\n\nstruct helper;\n";
        let parsed = parse(FileLang::Rust, src).unwrap();
        // `helper` names two same-file candidates (fn + struct): the ref in
        // `caller` is NULL. Pin 20's law.
        let call_ref = parsed
            .refs
            .iter()
            .find(|r| r.name == "helper" && r.start_line == 4)
            .expect("the call site is a ref");
        assert!(
            call_ref.symbol_index.is_none(),
            "two candidates in scope: NULL, never the first"
        );
        let unambiguous = "fn helper() {}\n\nfn caller() {\n    let _ = helper();\n}\n";
        let parsed = parse(FileLang::Rust, unambiguous).unwrap();
        let call_ref = parsed
            .refs
            .iter()
            .find(|r| r.name == "helper" && r.start_line == 4)
            .expect("the call site is a ref");
        assert!(call_ref.symbol_index.is_some(), "one candidate: aligned");
    }

    #[test]
    fn typescript_symbols_extract_with_dotted_qualified_names() {
        let src = "export function render(target: string): void {}\n\nexport class Panel {\n  mount(): void {}\n}\n\nexport interface Options {\n  label: string;\n}\n\nexport type Mode = 'quiet';\n";
        let parsed = parse(FileLang::TypeScript, src).unwrap();
        let names: Vec<&str> = parsed
            .symbols
            .iter()
            .map(|s| s.qualified_name.as_str())
            .collect();
        assert!(names.contains(&"render"), "{names:?}");
        assert!(names.contains(&"Panel"), "{names:?}");
        assert!(names.contains(&"Panel.mount"), "{names:?}");
        assert!(names.contains(&"Options"), "{names:?}");
        assert!(names.contains(&"Mode"), "{names:?}");
        let panel = parsed.symbols.iter().find(|s| s.name == "Panel").unwrap();
        assert_eq!(panel.kind, "class");
    }

    #[test]
    fn a_broken_file_still_yields_clean_symbols_and_reports_the_error() {
        let src = "fn fine() {}\nfn broken( {\n";
        let parsed = parse(FileLang::Rust, src).unwrap();
        assert!(parsed.has_error, "the ERROR node is reported");
        assert!(
            parsed.symbols.iter().any(|s| s.name == "fine"),
            "clean symbols before the break are still extracted"
        );
    }
}

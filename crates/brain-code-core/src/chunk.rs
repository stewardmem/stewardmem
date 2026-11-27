//! Chunking (S4): chunk by symbol; a windowed chunk only for top-level code
//! with no symbol, tagged `window` in `code_chunk.kind` so it can be counted
//! (§2 S4). Generated files never reach this module (pin 9). Token estimates
//! mirror the kernel's `est_tokens` law — chars/4 (src/search/packing.rs:149)
//! — re-derived here, not inherited: the crate stands alone.

use crate::hashing::sha256_hex;
use crate::parse::ParsedFile;

pub(crate) const CHARS_PER_TOKEN: usize = 4;

#[derive(Debug)]
pub struct ChunkDraft {
    /// `'symbol'` (a definition's slice) or `'window'` (top-level code with
    /// no symbol — imports, attributes, module-level statements). The tag is
    /// the countable difference.
    pub kind: &'static str,
    /// Index into the file's `ParsedFile.symbols` — `None` for windows.
    pub symbol_index: Option<usize>,
    pub text: String,
    pub token_estimate: usize,
    pub content_hash: String,
}

/// Every chunk for one parsed file: one slice per symbol, plus one chunk per
/// top-level stretch of code that belongs to no symbol.
pub fn chunks_for(src: &str, parsed: &ParsedFile) -> Vec<ChunkDraft> {
    let mut out = Vec::with_capacity(parsed.symbols.len() + parsed.nondef_top_ranges.len());
    for (index, symbol) in parsed.symbols.iter().enumerate() {
        let text = &src[symbol.start_byte..symbol.end_byte];
        out.push(ChunkDraft {
            kind: "symbol",
            symbol_index: Some(index),
            text: text.to_string(),
            token_estimate: text.len() / CHARS_PER_TOKEN,
            content_hash: sha256_hex(text.as_bytes()),
        });
    }
    for (start, end) in &parsed.nondef_top_ranges {
        let text = &src[*start..*end];
        out.push(ChunkDraft {
            kind: "window",
            symbol_index: None,
            text: text.to_string(),
            token_estimate: text.len() / CHARS_PER_TOKEN,
            content_hash: sha256_hex(text.as_bytes()),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::FileLang;
    use crate::parse;

    #[test]
    fn symbol_chunks_slice_their_definitions_and_windows_tag_the_rest() {
        let src =
            "use std::fmt;\n\nfn helper() -> u8 {\n    7\n}\n\nstruct Widget {\n    id: u8,\n}\n";
        let parsed = parse::parse(FileLang::Rust, src).unwrap();
        let chunks = chunks_for(src, &parsed);
        let symbols: Vec<&ChunkDraft> = chunks.iter().filter(|c| c.kind == "symbol").collect();
        let windows: Vec<&ChunkDraft> = chunks.iter().filter(|c| c.kind == "window").collect();
        assert_eq!(symbols.len(), 2, "one chunk per symbol: helper + Widget");
        assert!(symbols.iter().all(|c| c.symbol_index.is_some()));
        assert!(
            symbols.iter().any(|c| c.text.contains("fn helper")),
            "a symbol chunk is its definition's slice"
        );
        assert_eq!(
            windows.len(),
            1,
            "the use declaration is the only symbol-free top-level stretch"
        );
        assert_eq!(
            windows[0].kind, "window",
            "windows are tagged so they can be counted"
        );
        assert!(windows[0].symbol_index.is_none());
        assert_eq!(windows[0].text.trim(), "use std::fmt;");
        for chunk in &chunks {
            assert_eq!(chunk.token_estimate, chunk.text.len() / CHARS_PER_TOKEN);
            assert_eq!(chunk.content_hash.len(), 64);
        }
    }

    #[test]
    fn a_file_with_no_symbols_windows_whole_top_level_stretches() {
        let src = "const config = { quiet: true };\nexport default config;\n";
        let parsed = parse::parse(FileLang::TypeScript, src).unwrap();
        assert!(
            parsed.symbols.is_empty(),
            "expression statements are no definitions"
        );
        let chunks = chunks_for(src, &parsed);
        assert!(!chunks.is_empty());
        assert!(chunks.iter().all(|c| c.kind == "window"));
    }
}

use std::path::PathBuf;

use anyhow::Result;

const CHAR_CR: u8 = b'\r';
const CHAR_LF: u8 = b'\n';

#[derive(Debug)]
pub struct ParsedFile {
    pub path: PathBuf,
    pub source_text: String,
    pub syntax_tree: ::sv_parser::SyntaxTree,
}

#[derive(Debug, Clone, Default)]
pub struct ParseOptions {
    pub project_path: Option<PathBuf>,
    pub include_paths: Vec<PathBuf>,
}

pub fn get_pos_from_offset(src: &str, print_pos: usize) -> Option<(usize, usize)> {
    let mut pos = 0;
    let mut row = 1;
    let mut last_lf = None;
    let bytes = src.as_bytes();

    while pos < bytes.len() {
        if bytes[pos] == CHAR_LF {
            row += 1;
            last_lf = Some(pos);
        }

        if print_pos == pos {
            let column = if let Some(last_lf) = last_lf {
                pos - last_lf
            } else {
                pos + 1
            };
            let mut next_crlf = pos;
            while next_crlf < bytes.len() {
                if matches!(bytes[next_crlf], CHAR_CR | CHAR_LF) {
                    break;
                }
                next_crlf += 1;
            }

            return Some((column, row));
        }

        pos += 1;
    }

    None
}

pub fn row_from_offset(src: &str, offset: usize) -> Option<usize> {
    get_pos_from_offset(src, offset).map(|(_column, row)| row)
}

pub fn row_from_span(src: &str, offset: usize, len: usize) -> Option<usize> {
    let start_offset = offset.saturating_sub(len.saturating_sub(1));
    row_from_offset(src, start_offset)
}

pub fn get_original_lineno_from_ast_locate(
    tree: &::sv_parser::SyntaxTree,
    ast_locate: ::sv_parser::Locate,
    code_content: &str,
) -> Option<usize> {
    let (_, offset) = tree.get_origin(&ast_locate)?;
    get_pos_from_offset(code_content, offset).map(|(_column, row)| row)
}

pub trait AstProvider {
    fn parse_files(&self, paths: &[PathBuf]) -> Result<Vec<ParsedFile>>;
}

pub mod sv_parser;
pub use sv_parser::SvParserProvider;

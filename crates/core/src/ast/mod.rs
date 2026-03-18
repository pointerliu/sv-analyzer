use std::path::PathBuf;

use anyhow::Result;

#[derive(Debug)]
pub struct ParsedFile {
    pub path: PathBuf,
    pub source_text: String,
    pub syntax_tree: ::sv_parser::SyntaxTree,
}

pub trait AstProvider {
    fn parse_files(&self, paths: &[PathBuf]) -> Result<Vec<ParsedFile>>;
}

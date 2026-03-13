pub mod sv_parser;

use std::path::PathBuf;

use anyhow::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedFile {
    pub path: PathBuf,
}

pub trait AstProvider {
    fn parse_files(&self, paths: &[PathBuf]) -> Result<Vec<ParsedFile>>;
}

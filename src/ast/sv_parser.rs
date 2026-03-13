use std::collections::hash_map::RandomState;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use sv_parser::{parse_sv, Defines};

use super::{AstProvider, ParsedFile};

#[derive(Debug, Default)]
pub struct SvParserProvider;

impl AstProvider for SvParserProvider {
    fn parse_files(&self, paths: &[PathBuf]) -> Result<Vec<ParsedFile>> {
        paths.iter().map(parse_file).collect()
    }
}

fn parse_file(path: &PathBuf) -> Result<ParsedFile> {
    let source_text = fs::read_to_string(path).with_context(|| {
        format!(
            "failed to read SystemVerilog source from {}",
            path.display()
        )
    })?;
    let include_paths = path
        .parent()
        .map(|parent| vec![parent.to_path_buf()])
        .unwrap_or_default();
    let defines: Defines<RandomState> = Defines::default();
    let (syntax_tree, _) = parse_sv(path, &defines, &include_paths, false, false)
        .with_context(|| format!("failed to parse SystemVerilog file {}", path.display()))?;

    Ok(ParsedFile {
        path: path.clone(),
        source_text,
        syntax_tree,
    })
}

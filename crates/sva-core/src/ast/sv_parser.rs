use std::collections::hash_map::RandomState;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sv_parser::{parse_sv, Defines};

use crate::ast::{AstProvider, ParseOptions, ParsedFile};

#[derive(Debug, Default)]
pub struct SvParserProvider;

impl SvParserProvider {
    pub fn parse_files_with_options(
        &self,
        paths: &[PathBuf],
        options: &ParseOptions,
    ) -> Result<Vec<ParsedFile>> {
        paths.iter().map(|path| parse_file(path, options)).collect()
    }
}

impl AstProvider for SvParserProvider {
    fn parse_files(&self, paths: &[PathBuf]) -> Result<Vec<ParsedFile>> {
        self.parse_files_with_options(paths, &ParseOptions::default())
    }
}

fn parse_file(path: &PathBuf, options: &ParseOptions) -> Result<ParsedFile> {
    let source_text = fs::read_to_string(path).with_context(|| {
        format!(
            "failed to read SystemVerilog source from {}",
            path.display()
        )
    })?;
    let include_paths = build_include_paths(path, options);
    let defines = default_defines();
    let (syntax_tree, _) = parse_sv(path, &defines, &include_paths, false, false)
        .with_context(|| format!("failed to parse SystemVerilog file {}", path.display()))?;

    Ok(ParsedFile {
        path: path.clone(),
        source_text,
        syntax_tree,
    })
}

fn build_include_paths(path: &Path, options: &ParseOptions) -> Vec<PathBuf> {
    let mut include_paths = Vec::new();

    if let Some(parent) = path.parent() {
        push_unique_path(&mut include_paths, parent.to_path_buf());
    }

    if let Some(project_include_path) = project_include_path(options.project_path.as_deref()) {
        push_unique_path(&mut include_paths, project_include_path);
    }

    for include_path in &options.include_paths {
        push_unique_path(&mut include_paths, include_path.clone());
    }

    include_paths
}

fn project_include_path(project_path: Option<&Path>) -> Option<PathBuf> {
    let project_path = project_path?;

    if project_path.is_dir() {
        Some(project_path.to_path_buf())
    } else {
        project_path.parent().map(Path::to_path_buf)
    }
}

fn push_unique_path(include_paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !include_paths.iter().any(|existing| existing == &path) {
        include_paths.push(path);
    }
}

fn default_defines() -> Defines<RandomState> {
    [("RVFI".to_string(), None)].into_iter().collect()
}

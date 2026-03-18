use std::collections::{BTreeSet, HashMap};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::block::{Block, BlockType, CircuitType, DataflowBlockizer};
use crate::wave::WellenReader;
use dac26_core::ast::ParsedFile;
use dac26_core::block::Blockizer;
use dac26_core::types::{SignalNode, Timestamp};
use dac26_core::wave::WaveformReader;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatementCoverageEntry {
    pub file: String,
    pub line: usize,
    pub code_snippet: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatementCoverageReport {
    pub time: i64,
    pub covered: Vec<StatementCoverageEntry>,
    pub uncovered: Vec<StatementCoverageEntry>,
}

#[derive(Debug, Clone)]
struct AssignmentStatement {
    file: String,
    line: usize,
    code_snippet: String,
    block: Block,
    guards: Vec<Guard>,
    triggers: Vec<(String, EdgeKind)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Guard {
    Predicate(String),
    NotPredicate(String),
    CaseMatch {
        selector: String,
        items: Vec<String>,
    },
    CaseDefault {
        selector: String,
        items: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EdgeKind {
    Posedge,
    Negedge,
}

#[derive(Debug, Default)]
struct SignalResolver {
    prefixes_by_file: HashMap<String, Vec<String>>,
}

impl SignalResolver {
    fn new(wave: &WellenReader) -> Self {
        let mut prefixes_by_file: HashMap<String, BTreeSet<String>> = HashMap::new();

        for name in wave.signal_names() {
            let Some((prefix, trace)) = name.rsplit_once(".vlCoverageLineTrace_") else {
                continue;
            };
            let Some((file, _)) = trace.split_once("__") else {
                continue;
            };
            prefixes_by_file
                .entry(file.to_string())
                .or_default()
                .insert(prefix.to_string());
        }

        Self {
            prefixes_by_file: prefixes_by_file
                .into_iter()
                .map(|(file, prefixes)| (file, prefixes.into_iter().collect()))
                .collect(),
        }
    }

    fn signal_value_bits(
        &self,
        file: &str,
        signal: &str,
        wave: &WellenReader,
        time: Timestamp,
    ) -> Result<Option<String>> {
        for candidate in self.candidates(file, signal) {
            if let Some(value) = wave.signal_value_at(&SignalNode::named(candidate), time)? {
                return Ok(Some(value.raw_bits));
            }
        }

        Ok(None)
    }

    fn candidates(&self, file: &str, signal: &str) -> Vec<String> {
        let mut candidates = Vec::new();
        if let Some(prefixes) = self.prefixes_by_file.get(file) {
            for prefix in prefixes.iter().rev() {
                candidates.push(format!("{prefix}.{signal}"));
            }
        }
        candidates.push(signal.to_string());
        candidates
    }
}

pub fn assignment_statement_coverage_report(
    parsed_files: &[ParsedFile],
    wave: &WellenReader,
    time: Timestamp,
) -> Result<StatementCoverageReport> {
    let block_set = DataflowBlockizer.blockize(parsed_files)?;
    let blocks_by_location = block_set
        .blocks()
        .iter()
        .map(|block| {
            (
                (
                    normalize_file_key(block.source_file()),
                    block.line_start(),
                    block.line_end(),
                ),
                block.clone(),
            )
        })
        .collect::<HashMap<_, _>>();
    let constants = collect_named_constants(parsed_files);
    let guard_map = collect_assignment_guards(parsed_files);
    let triggers_by_location = collect_block_triggers(parsed_files);
    let resolver = SignalResolver::new(wave);
    let statements = collect_assignment_statements(
        parsed_files,
        &blocks_by_location,
        &guard_map,
        &triggers_by_location,
    )?;

    let mut covered = Vec::new();
    let mut uncovered = Vec::new();

    for statement in statements {
        let entry = StatementCoverageEntry {
            file: statement.file.clone(),
            line: statement.line,
            code_snippet: statement.code_snippet.clone(),
        };

        if statement_is_covered(&statement, wave, time, &constants, &resolver)? {
            covered.push(entry);
        } else {
            uncovered.push(entry);
        }
    }

    covered.sort_by_key(|entry| (entry.file.clone(), entry.line));
    uncovered.sort_by_key(|entry| (entry.file.clone(), entry.line));

    Ok(StatementCoverageReport {
        time: time.0,
        covered,
        uncovered,
    })
}

fn statement_is_covered(
    statement: &AssignmentStatement,
    wave: &WellenReader,
    time: Timestamp,
    constants: &HashMap<String, String>,
    resolver: &SignalResolver,
) -> Result<bool> {
    match statement.block.block_type() {
        BlockType::Assign => block_inputs_changed(&statement.block, wave, time, resolver),
        BlockType::Always => match statement.block.circuit_type() {
            CircuitType::Combinational => {
                if !block_inputs_changed(&statement.block, wave, time, resolver)? {
                    return Ok(false);
                }
                evaluate_guards(
                    &statement.guards,
                    &statement.file,
                    wave,
                    time,
                    constants,
                    resolver,
                )
            }
            CircuitType::Sequential => {
                let previous_time = Timestamp(time.0 - 1);
                if previous_time.0 < 0 {
                    return Ok(false);
                }
                if !sequential_block_triggered(
                    &statement.file,
                    &statement.triggers,
                    wave,
                    previous_time,
                    time,
                    resolver,
                )? {
                    return Ok(false);
                }
                evaluate_guards(
                    &statement.guards,
                    &statement.file,
                    wave,
                    previous_time,
                    constants,
                    resolver,
                )
            }
        },
        _ => Ok(false),
    }
}

fn evaluate_guards(
    guards: &[Guard],
    file: &str,
    wave: &WellenReader,
    time: Timestamp,
    constants: &HashMap<String, String>,
    resolver: &SignalResolver,
) -> Result<bool> {
    for guard in guards {
        let matches = match guard {
            Guard::Predicate(expr) => eval_predicate(expr, file, wave, time, constants, resolver)?,
            Guard::NotPredicate(expr) => {
                !eval_predicate(expr, file, wave, time, constants, resolver)?
            }
            Guard::CaseMatch { selector, items } => {
                let Some(selector_bits) =
                    value_bits(selector, file, wave, time, constants, resolver)?
                else {
                    return Ok(false);
                };
                let mut matched = false;
                for item in items {
                    if let Some(item_bits) =
                        value_bits(item, file, wave, time, constants, resolver)?
                    {
                        if bits_equal(&selector_bits, &item_bits) {
                            matched = true;
                            break;
                        }
                    }
                }
                matched
            }
            Guard::CaseDefault { selector, items } => {
                let Some(selector_bits) =
                    value_bits(selector, file, wave, time, constants, resolver)?
                else {
                    return Ok(false);
                };
                let mut matched = false;
                for item in items {
                    if let Some(item_bits) =
                        value_bits(item, file, wave, time, constants, resolver)?
                    {
                        if bits_equal(&selector_bits, &item_bits) {
                            matched = true;
                            break;
                        }
                    }
                }
                !matched
            }
        };

        if !matches {
            return Ok(false);
        }
    }

    Ok(true)
}

fn eval_predicate(
    expr: &str,
    file: &str,
    wave: &WellenReader,
    time: Timestamp,
    constants: &HashMap<String, String>,
    resolver: &SignalResolver,
) -> Result<bool> {
    let expr = trim_wrapping_parens(expr.trim());

    if let Some(rest) = expr.strip_prefix('!') {
        return Ok(!eval_predicate(
            rest.trim(),
            file,
            wave,
            time,
            constants,
            resolver,
        )?);
    }
    if let Some((left, right)) = split_once_operator(expr, "==") {
        return compare_values(left, right, file, wave, time, constants, resolver, true);
    }
    if let Some((left, right)) = split_once_operator(expr, "!=") {
        return compare_values(left, right, file, wave, time, constants, resolver, false);
    }

    let Some(bits) = value_bits(expr, file, wave, time, constants, resolver)? else {
        return Ok(false);
    };
    Ok(bits.chars().any(|bit| bit == '1'))
}

fn compare_values(
    left: &str,
    right: &str,
    file: &str,
    wave: &WellenReader,
    time: Timestamp,
    constants: &HashMap<String, String>,
    resolver: &SignalResolver,
    expect_equal: bool,
) -> Result<bool> {
    let Some(left_bits) = value_bits(left, file, wave, time, constants, resolver)? else {
        return Ok(false);
    };
    let Some(right_bits) = value_bits(right, file, wave, time, constants, resolver)? else {
        return Ok(false);
    };
    Ok(bits_equal(&left_bits, &right_bits) == expect_equal)
}

fn value_bits(
    expr: &str,
    file: &str,
    wave: &WellenReader,
    time: Timestamp,
    constants: &HashMap<String, String>,
    resolver: &SignalResolver,
) -> Result<Option<String>> {
    let expr = trim_wrapping_parens(expr.trim());

    if let Some(bits) = parse_sv_literal(expr) {
        return Ok(Some(bits));
    }
    if let Some(bits) = constants.get(expr) {
        return Ok(Some(bits.clone()));
    }

    resolver.signal_value_bits(file, expr, wave, time)
}

fn bits_equal(left: &str, right: &str) -> bool {
    let width = left.len().max(right.len());
    left.chars()
        .rev()
        .chain(std::iter::repeat('0'))
        .take(width)
        .eq(right
            .chars()
            .rev()
            .chain(std::iter::repeat('0'))
            .take(width))
}

fn block_inputs_changed(
    block: &Block,
    wave: &WellenReader,
    time: Timestamp,
    resolver: &SignalResolver,
) -> Result<bool> {
    let previous_time = Timestamp(time.0 - 1);
    if previous_time.0 < 0 {
        return Ok(true);
    }

    let file = normalize_file_key(block.source_file());
    for signal in block.input_signals() {
        if !signal.is_variable() {
            continue;
        }
        let current = resolver.signal_value_bits(&file, signal.as_str(), wave, time)?;
        let previous = resolver.signal_value_bits(&file, signal.as_str(), wave, previous_time)?;
        if current.is_some() && current != previous {
            return Ok(true);
        }
    }

    Ok(false)
}

fn sequential_block_triggered(
    file: &str,
    triggers: &[(String, EdgeKind)],
    wave: &WellenReader,
    previous_time: Timestamp,
    time: Timestamp,
    resolver: &SignalResolver,
) -> Result<bool> {
    for (signal, edge) in triggers {
        let current = resolver.signal_value_bits(file, &signal, wave, time)?;
        let previous = resolver.signal_value_bits(file, &signal, wave, previous_time)?;
        if edge_matches(previous.as_deref(), current.as_deref(), *edge) {
            return Ok(true);
        }
    }

    Ok(false)
}

fn collect_block_triggers(
    parsed_files: &[ParsedFile],
) -> HashMap<(String, usize, usize), Vec<(String, EdgeKind)>> {
    let mut triggers_by_block = HashMap::new();

    for file in parsed_files {
        let normalized_file = normalize_file_key(&file.path.display().to_string());
        for event in (&file.syntax_tree).into_iter().event() {
            if let sv_parser::NodeEvent::Enter(sv_parser::RefNode::AlwaysConstruct(
                always_construct,
            )) = event
            {
                let line_start = locate_line_from_node(always_construct.into()).unwrap_or(1);
                let line_end = statement_end_line(&always_construct.nodes.1).unwrap_or(line_start);
                triggers_by_block.insert(
                    (normalized_file.clone(), line_start, line_end),
                    always_construct_triggers(&file.syntax_tree, always_construct),
                );
            }
        }
    }

    triggers_by_block
}

fn always_construct_triggers(
    syntax_tree: &sv_parser::SyntaxTree,
    always_construct: &sv_parser::AlwaysConstruct,
) -> Vec<(String, EdgeKind)> {
    match &always_construct.nodes.1.nodes.2 {
        sv_parser::StatementItem::ProceduralTimingControlStatement(statement) => {
            timing_control_triggers(syntax_tree, &statement.nodes.0)
        }
        _ => Vec::new(),
    }
}

fn timing_control_triggers(
    syntax_tree: &sv_parser::SyntaxTree,
    control: &sv_parser::ProceduralTimingControl,
) -> Vec<(String, EdgeKind)> {
    match control {
        sv_parser::ProceduralTimingControl::EventControl(control) => {
            event_control_triggers(syntax_tree, control)
        }
        _ => Vec::new(),
    }
}

fn event_control_triggers(
    syntax_tree: &sv_parser::SyntaxTree,
    control: &sv_parser::EventControl,
) -> Vec<(String, EdgeKind)> {
    match control {
        sv_parser::EventControl::EventExpression(control) => {
            event_expression_triggers(syntax_tree, &control.nodes.1.nodes.1)
        }
        _ => Vec::new(),
    }
}

fn event_expression_triggers(
    syntax_tree: &sv_parser::SyntaxTree,
    expression: &sv_parser::EventExpression,
) -> Vec<(String, EdgeKind)> {
    match expression {
        sv_parser::EventExpression::Expression(expression) => {
            let Some(edge_identifier) = &expression.nodes.0 else {
                return Vec::new();
            };
            let edge = match syntax_tree.get_str(edge_identifier).map(str::trim) {
                Some("posedge") => EdgeKind::Posedge,
                Some("negedge") => EdgeKind::Negedge,
                _ => return Vec::new(),
            };
            let signal = syntax_tree
                .get_str(&expression.nodes.1)
                .map(str::trim)
                .unwrap_or_default()
                .to_string();
            vec![(signal, edge)]
        }
        sv_parser::EventExpression::Or(expression) => {
            let mut left = event_expression_triggers(syntax_tree, &expression.nodes.0);
            left.extend(event_expression_triggers(syntax_tree, &expression.nodes.2));
            left
        }
        sv_parser::EventExpression::Comma(expression) => {
            let mut left = event_expression_triggers(syntax_tree, &expression.nodes.0);
            left.extend(event_expression_triggers(syntax_tree, &expression.nodes.2));
            left
        }
        sv_parser::EventExpression::Paren(expression) => {
            event_expression_triggers(syntax_tree, &expression.nodes.0.nodes.1)
        }
        _ => Vec::new(),
    }
}

fn edge_matches(previous: Option<&str>, current: Option<&str>, edge: EdgeKind) -> bool {
    let Some(previous) = previous else {
        return false;
    };
    let Some(current) = current else {
        return false;
    };
    match edge {
        EdgeKind::Posedge => previous == "0" && current == "1",
        EdgeKind::Negedge => previous == "1" && current == "0",
    }
}

fn collect_assignment_statements(
    parsed_files: &[ParsedFile],
    blocks_by_location: &HashMap<(String, usize, usize), Block>,
    guard_map: &HashMap<(String, usize), Vec<Guard>>,
    triggers_by_location: &HashMap<(String, usize, usize), Vec<(String, EdgeKind)>>,
) -> Result<Vec<AssignmentStatement>> {
    let mut statements = Vec::new();

    for file in parsed_files {
        let normalized_file = normalize_file_key(&file.path.display().to_string());
        let line_snippets = file.source_text.lines().collect::<Vec<_>>();
        let parsed = DataflowBlockizer.blockize(std::slice::from_ref(file))?;

        for block in parsed.blocks() {
            if !matches!(block.block_type(), BlockType::Assign | BlockType::Always) {
                continue;
            }

            let Some(full_block) = blocks_by_location
                .get(&(
                    normalized_file.clone(),
                    block.line_start(),
                    block.line_end(),
                ))
                .cloned()
            else {
                continue;
            };

            let mut seen_lines = BTreeSet::new();
            let mut block_statements = Vec::new();
            let triggers = triggers_by_location
                .get(&(
                    normalized_file.clone(),
                    block.line_start(),
                    block.line_end(),
                ))
                .cloned()
                .unwrap_or_default();
            for entry in block.dataflow() {
                let Some(output) = entry.output.first() else {
                    continue;
                };
                let line = output.locate.line;
                if line == 0 || !seen_lines.insert(line) {
                    continue;
                }

                block_statements.push(AssignmentStatement {
                    file: normalized_file.clone(),
                    line,
                    code_snippet: line_snippets
                        .get(line.saturating_sub(1))
                        .map(|line| line.trim().to_string())
                        .unwrap_or_default(),
                    block: full_block.clone(),
                    guards: guard_map
                        .get(&(normalized_file.clone(), line))
                        .cloned()
                        .unwrap_or_default(),
                    triggers: triggers.clone(),
                });
            }
            statements.extend(block_statements);
        }
    }

    statements.sort_by_key(|statement| (statement.file.clone(), statement.line));
    Ok(statements)
}

fn collect_assignment_guards(parsed_files: &[ParsedFile]) -> HashMap<(String, usize), Vec<Guard>> {
    let mut guards_by_line = HashMap::new();

    for file in parsed_files {
        let normalized_file = normalize_file_key(&file.path.display().to_string());
        for event in (&file.syntax_tree).into_iter().event() {
            if let sv_parser::NodeEvent::Enter(sv_parser::RefNode::AlwaysConstruct(
                always_construct,
            )) = event
            {
                let mut guards = Vec::new();
                collect_statement_guards(
                    &file.syntax_tree,
                    &always_construct.nodes.1,
                    &mut guards,
                    &normalized_file,
                    &mut guards_by_line,
                );
            }
        }
    }

    guards_by_line
}

fn collect_statement_guards(
    syntax_tree: &sv_parser::SyntaxTree,
    statement: &sv_parser::Statement,
    guards: &mut Vec<Guard>,
    file: &str,
    guards_by_line: &mut HashMap<(String, usize), Vec<Guard>>,
) {
    match &statement.nodes.2 {
        sv_parser::StatementItem::BlockingAssignment(_)
        | sv_parser::StatementItem::NonblockingAssignment(_) => {
            let line = statement_assignment_line(&statement.nodes.2).unwrap_or(0);
            if line != 0 {
                guards_by_line.insert((file.to_string(), line), guards.clone());
            }
        }
        sv_parser::StatementItem::SeqBlock(block) => {
            for declaration in &block.nodes.2 {
                collect_block_item_declaration_guards(declaration, guards, file, guards_by_line);
            }
            for statement in &block.nodes.3 {
                if let sv_parser::StatementOrNull::Statement(statement) = statement {
                    collect_statement_guards(syntax_tree, statement, guards, file, guards_by_line);
                }
            }
        }
        sv_parser::StatementItem::ProceduralTimingControlStatement(statement) => {
            if let sv_parser::StatementOrNull::Statement(statement) = &statement.nodes.1 {
                collect_statement_guards(syntax_tree, statement, guards, file, guards_by_line);
            }
        }
        sv_parser::StatementItem::ConditionalStatement(conditional) => {
            let predicate = syntax_tree
                .get_str(&conditional.nodes.2.nodes.1)
                .map(str::trim)
                .unwrap_or_default()
                .to_string();

            guards.push(Guard::Predicate(predicate.clone()));
            if let sv_parser::StatementOrNull::Statement(statement) = &conditional.nodes.3 {
                collect_statement_guards(syntax_tree, statement, guards, file, guards_by_line);
            }
            guards.pop();

            let mut prior = vec![predicate];
            for (_, _, predicate_group, statement) in &conditional.nodes.4 {
                let predicate = syntax_tree
                    .get_str(&predicate_group.nodes.1)
                    .map(str::trim)
                    .unwrap_or_default()
                    .to_string();
                for previous in &prior {
                    guards.push(Guard::NotPredicate(previous.clone()));
                }
                guards.push(Guard::Predicate(predicate.clone()));
                if let sv_parser::StatementOrNull::Statement(statement) = statement {
                    collect_statement_guards(syntax_tree, statement, guards, file, guards_by_line);
                }
                guards.pop();
                for _ in &prior {
                    guards.pop();
                }
                prior.push(predicate);
            }

            if let Some((_, statement)) = &conditional.nodes.5 {
                for previous in &prior {
                    guards.push(Guard::NotPredicate(previous.clone()));
                }
                if let sv_parser::StatementOrNull::Statement(statement) = statement {
                    collect_statement_guards(syntax_tree, statement, guards, file, guards_by_line);
                }
                for _ in &prior {
                    guards.pop();
                }
            }
        }
        sv_parser::StatementItem::CaseStatement(case_statement) => {
            let sv_parser::CaseStatement::Normal(case_statement) = &**case_statement else {
                return;
            };
            let selector = syntax_tree
                .get_str(&case_statement.nodes.2.nodes.1.nodes.0)
                .map(str::trim)
                .unwrap_or_default()
                .to_string();
            let mut seen_items = Vec::new();
            for item in
                std::iter::once(&case_statement.nodes.3).chain(case_statement.nodes.4.iter())
            {
                match item {
                    sv_parser::CaseItem::NonDefault(item) => {
                        let items = item
                            .nodes
                            .0
                            .contents()
                            .into_iter()
                            .filter_map(|expr| {
                                syntax_tree
                                    .get_str(&expr.nodes.0)
                                    .map(str::trim)
                                    .map(str::to_string)
                            })
                            .collect::<Vec<_>>();
                        guards.push(Guard::CaseMatch {
                            selector: selector.clone(),
                            items: items.clone(),
                        });
                        if let sv_parser::StatementOrNull::Statement(statement) = &item.nodes.2 {
                            collect_statement_guards(
                                syntax_tree,
                                statement,
                                guards,
                                file,
                                guards_by_line,
                            );
                        }
                        guards.pop();
                        seen_items.extend(items);
                    }
                    sv_parser::CaseItem::Default(item) => {
                        guards.push(Guard::CaseDefault {
                            selector: selector.clone(),
                            items: seen_items.clone(),
                        });
                        if let sv_parser::StatementOrNull::Statement(statement) = &item.nodes.2 {
                            collect_statement_guards(
                                syntax_tree,
                                statement,
                                guards,
                                file,
                                guards_by_line,
                            );
                        }
                        guards.pop();
                    }
                }
            }
        }
        _ => {}
    }
}

fn collect_block_item_declaration_guards(
    declaration: &sv_parser::BlockItemDeclaration,
    guards: &[Guard],
    file: &str,
    guards_by_line: &mut HashMap<(String, usize), Vec<Guard>>,
) {
    let sv_parser::BlockItemDeclaration::Data(declaration) = declaration else {
        return;
    };
    let sv_parser::DataDeclaration::Variable(declaration) = &declaration.nodes.1 else {
        return;
    };

    for assignment in declaration.nodes.4.nodes.0.contents() {
        let sv_parser::VariableDeclAssignment::Variable(assignment) = assignment else {
            continue;
        };
        let line = locate_line_from_node((&assignment.nodes.0).into()).unwrap_or(0);
        if line != 0 {
            guards_by_line.insert((file.to_string(), line), guards.to_vec());
        }
    }
}

fn collect_named_constants(parsed_files: &[ParsedFile]) -> HashMap<String, String> {
    let mut constants = HashMap::new();

    for file in parsed_files {
        for line in file.source_text.lines() {
            let Some((name, value)) = line.split_once('=') else {
                continue;
            };
            let name = name
                .split_whitespace()
                .last()
                .unwrap_or_default()
                .trim_matches(',')
                .trim();
            let value = value
                .split(|ch: char| matches!(ch, ',' | ';' | '}'))
                .next()
                .unwrap_or_default()
                .trim();
            if name.is_empty() {
                continue;
            }
            if let Some(bits) = parse_sv_literal(value) {
                constants.insert(name.to_string(), bits);
            }
        }
    }

    constants
}

fn parse_sv_literal(text: &str) -> Option<String> {
    let text = text.trim().replace('_', "");
    let (width_text, rest) = text.split_once('\'')?;
    let width = width_text.parse::<usize>().ok()?;
    let mut chars = rest.chars();
    let base = chars.next()?.to_ascii_lowercase();
    let digits = chars.collect::<String>();
    let value = match base {
        'b' => digits,
        'h' => digits
            .chars()
            .filter_map(|ch| ch.to_digit(16))
            .map(|digit| format!("{digit:04b}"))
            .collect::<String>(),
        'd' => format!("{:b}", digits.parse::<u128>().ok()?),
        _ => return None,
    };

    if value.len() >= width {
        Some(value[value.len() - width..].to_string())
    } else {
        Some(format!("{}{}", "0".repeat(width - value.len()), value))
    }
}

fn split_once_operator<'a>(text: &'a str, operator: &str) -> Option<(&'a str, &'a str)> {
    let (left, right) = text.split_once(operator)?;
    Some((left.trim(), right.trim()))
}

fn trim_wrapping_parens(mut text: &str) -> &str {
    loop {
        let trimmed = text.trim();
        if !(trimmed.starts_with('(') && trimmed.ends_with(')')) {
            return trimmed;
        }
        text = &trimmed[1..trimmed.len() - 1];
    }
}

fn normalize_file_key(file: &str) -> String {
    std::path::Path::new(file)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(file)
        .to_string()
}

fn statement_assignment_line(statement: &sv_parser::StatementItem) -> Option<usize> {
    match statement {
        sv_parser::StatementItem::BlockingAssignment(assignment) => {
            blocking_assignment_line(&assignment.0)
        }
        sv_parser::StatementItem::NonblockingAssignment(assignment) => {
            variable_lvalue_line(&assignment.0.nodes.0)
        }
        _ => None,
    }
}

fn blocking_assignment_line(assignment: &sv_parser::BlockingAssignment) -> Option<usize> {
    match assignment {
        sv_parser::BlockingAssignment::Variable(assignment) => {
            variable_lvalue_line(&assignment.nodes.0)
        }
        sv_parser::BlockingAssignment::OperatorAssignment(assignment) => {
            variable_lvalue_line(&assignment.nodes.0)
        }
        sv_parser::BlockingAssignment::HierarchicalVariable(assignment) => {
            locate_line_from_node((&assignment.nodes.1).into())
        }
        sv_parser::BlockingAssignment::NonrangeVariable(assignment) => {
            locate_line_from_node((&assignment.nodes.1).into())
        }
    }
}

fn variable_lvalue_line(lvalue: &sv_parser::VariableLvalue) -> Option<usize> {
    match lvalue {
        sv_parser::VariableLvalue::Identifier(identifier) => {
            locate_line_from_node((&identifier.nodes.1).into())
        }
        sv_parser::VariableLvalue::Lvalue(lvalues) => lvalues
            .nodes
            .0
            .nodes
            .1
            .contents()
            .into_iter()
            .find_map(variable_lvalue_line),
        _ => None,
    }
}

fn locate_line_from_node(node: sv_parser::RefNode) -> Option<usize> {
    sv_parser::unwrap_locate!(node).and_then(|loc| usize::try_from(loc.line).ok())
}

fn statement_end_line(statement: &sv_parser::Statement) -> Option<usize> {
    match &statement.nodes.2 {
        sv_parser::StatementItem::SeqBlock(block) => keyword_line(&block.nodes.4),
        sv_parser::StatementItem::ProceduralTimingControlStatement(statement) => {
            statement_or_null_end_line(&statement.nodes.1)
        }
        sv_parser::StatementItem::BlockingAssignment(assignment) => symbol_line(&assignment.1),
        sv_parser::StatementItem::NonblockingAssignment(assignment) => symbol_line(&assignment.1),
        sv_parser::StatementItem::ConditionalStatement(statement) => {
            conditional_end_line(statement)
        }
        sv_parser::StatementItem::CaseStatement(statement) => case_statement_end_line(statement),
        _ => locate_line_from_node(statement.into()),
    }
}

fn statement_or_null_end_line(statement: &sv_parser::StatementOrNull) -> Option<usize> {
    match statement {
        sv_parser::StatementOrNull::Statement(statement) => statement_end_line(statement),
        sv_parser::StatementOrNull::Attribute(attribute) => symbol_line(&attribute.nodes.1),
    }
}

fn conditional_end_line(statement: &sv_parser::ConditionalStatement) -> Option<usize> {
    if let Some((_, tail)) = &statement.nodes.5 {
        return statement_or_null_end_line(tail);
    }
    if let Some((_, _, _, tail)) = statement.nodes.4.last() {
        return statement_or_null_end_line(tail);
    }
    statement_or_null_end_line(&statement.nodes.3)
}

fn case_statement_end_line(statement: &sv_parser::CaseStatement) -> Option<usize> {
    match statement {
        sv_parser::CaseStatement::Normal(statement) => keyword_line(&statement.nodes.5),
        _ => locate_line_from_node(statement.into()),
    }
}

fn keyword_line(keyword: &sv_parser::Keyword) -> Option<usize> {
    usize::try_from(keyword.nodes.0.line).ok()
}

fn symbol_line(symbol: &sv_parser::Symbol) -> Option<usize> {
    usize::try_from(symbol.nodes.0.line).ok()
}

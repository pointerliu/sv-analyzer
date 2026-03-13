use std::collections::HashSet;

use anyhow::Result;
use sv_parser::{unwrap_locate, unwrap_node, RefNode};

use super::{Block, BlockSet, BlockType, Blockizer, CircuitType, DataflowEntry};
use crate::ast::ParsedFile;
use crate::types::BlockId;

#[derive(Debug, Default)]
pub struct DataflowBlockizer;

impl Blockizer for DataflowBlockizer {
    fn blockize(&self, files: &[ParsedFile]) -> Result<BlockSet> {
        let mut collector = BlockCollector::default();

        for file in files {
            collector.collect_file(file)?;
        }

        BlockSet::new(collector.blocks)
    }
}

#[derive(Default)]
struct BlockCollector {
    next_block_id: u64,
    blocks: Vec<Block>,
}

impl BlockCollector {
    fn collect_file(&mut self, file: &ParsedFile) -> Result<()> {
        let mut current_module = None;

        for event in (&file.syntax_tree).into_iter().event() {
            match event {
                sv_parser::NodeEvent::Enter(RefNode::ModuleDeclarationAnsi(module)) => {
                    current_module = module_name_from_node(&file.syntax_tree, module.into());
                }
                sv_parser::NodeEvent::Enter(RefNode::ModuleDeclarationNonansi(module)) => {
                    current_module = module_name_from_node(&file.syntax_tree, module.into());
                }
                sv_parser::NodeEvent::Leave(RefNode::ModuleDeclarationAnsi(_))
                | sv_parser::NodeEvent::Leave(RefNode::ModuleDeclarationNonansi(_)) => {
                    current_module = None;
                }
                sv_parser::NodeEvent::Enter(RefNode::ContinuousAssign(assign)) => {
                    if let Some(module_scope) = current_module.as_deref() {
                        let dataflow =
                            extract_continuous_assign_dataflow(&file.syntax_tree, assign.into());
                        if !dataflow.is_empty() {
                            self.push_block(
                                file,
                                module_scope,
                                BlockType::Assign,
                                CircuitType::Combinational,
                                assign.into(),
                                dataflow,
                            )?;
                        }
                    }
                }
                sv_parser::NodeEvent::Enter(RefNode::AlwaysConstruct(always_construct)) => {
                    if let Some(module_scope) = current_module.as_deref() {
                        let (block_type, circuit_type) = classify_always(always_construct);
                        let dataflow = extract_always_dataflow(&file.syntax_tree, always_construct);
                        if !dataflow.is_empty() {
                            self.push_block(
                                file,
                                module_scope,
                                block_type,
                                circuit_type,
                                always_construct.into(),
                                dataflow,
                            )?;
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn push_block(
        &mut self,
        file: &ParsedFile,
        module_scope: &str,
        block_type: BlockType,
        circuit_type: CircuitType,
        node: RefNode,
        dataflow: Vec<DataflowEntry>,
    ) -> Result<()> {
        let line = unwrap_locate!(node)
            .map(|loc| usize::try_from(loc.line).unwrap_or(1))
            .unwrap_or(1);

        self.blocks.push(Block::new(
            BlockId(self.next_block_id),
            block_type,
            circuit_type,
            module_scope,
            file.path.display().to_string(),
            line,
            line,
            dataflow,
            String::new(),
        )?);
        self.next_block_id += 1;

        Ok(())
    }
}

fn classify_always(always_construct: &sv_parser::AlwaysConstruct) -> (BlockType, CircuitType) {
    match &always_construct.nodes.0 {
        sv_parser::AlwaysKeyword::AlwaysFf(_) => (BlockType::Always, CircuitType::Sequential),
        sv_parser::AlwaysKeyword::AlwaysLatch(_) => (BlockType::Always, CircuitType::Sequential),
        sv_parser::AlwaysKeyword::Always(_) if has_edge_trigger(&always_construct.nodes.1) => {
            (BlockType::Always, CircuitType::Sequential)
        }
        _ => (BlockType::Always, CircuitType::Combinational),
    }
}

fn has_edge_trigger(statement: &sv_parser::Statement) -> bool {
    match &statement.nodes.2 {
        sv_parser::StatementItem::ProceduralTimingControlStatement(statement) => {
            has_edge_in_timing_control(&statement.nodes.0)
        }
        _ => false,
    }
}

fn has_edge_in_timing_control(control: &sv_parser::ProceduralTimingControl) -> bool {
    match control {
        sv_parser::ProceduralTimingControl::EventControl(control) => {
            has_edge_in_event_control(control)
        }
        _ => false,
    }
}

fn has_edge_in_event_control(control: &sv_parser::EventControl) -> bool {
    match control {
        sv_parser::EventControl::EventExpression(control) => {
            has_edge_in_event_expression(&control.nodes.1.nodes.1)
        }
        _ => false,
    }
}

fn has_edge_in_event_expression(expression: &sv_parser::EventExpression) -> bool {
    match expression {
        sv_parser::EventExpression::Expression(expression) => expression.nodes.0.is_some(),
        sv_parser::EventExpression::Or(expression) => {
            has_edge_in_event_expression(&expression.nodes.0)
                || has_edge_in_event_expression(&expression.nodes.2)
        }
        sv_parser::EventExpression::Comma(expression) => {
            has_edge_in_event_expression(&expression.nodes.0)
                || has_edge_in_event_expression(&expression.nodes.2)
        }
        sv_parser::EventExpression::Paren(expression) => {
            has_edge_in_event_expression(&expression.nodes.0.nodes.1)
        }
        _ => false,
    }
}

fn module_name_from_node(syntax_tree: &sv_parser::SyntaxTree, node: RefNode) -> Option<String> {
    let id = unwrap_node!(node, ModuleIdentifier)?;
    Some(identifier_text(syntax_tree, id))
}

fn identifier_text(syntax_tree: &sv_parser::SyntaxTree, node: RefNode) -> String {
    match unwrap_node!(node, SimpleIdentifier, EscapedIdentifier) {
        Some(RefNode::SimpleIdentifier(id)) => syntax_tree
            .get_str(&id.nodes.0)
            .unwrap_or_default()
            .to_string(),
        Some(RefNode::EscapedIdentifier(id)) => syntax_tree
            .get_str(&id.nodes.0)
            .unwrap_or_default()
            .trim()
            .to_string(),
        _ => String::new(),
    }
}

fn hierarchical_variable_identifier_text(
    syntax_tree: &sv_parser::SyntaxTree,
    node: &sv_parser::HierarchicalVariableIdentifier,
) -> String {
    syntax_tree
        .get_str(node)
        .map(str::trim)
        .unwrap_or_default()
        .to_string()
}

fn ps_or_hierarchical_net_identifier_text(
    syntax_tree: &sv_parser::SyntaxTree,
    node: &sv_parser::PsOrHierarchicalNetIdentifier,
) -> String {
    syntax_tree
        .get_str(node)
        .map(str::trim)
        .unwrap_or_default()
        .to_string()
}

fn hierarchical_identifier_text(
    syntax_tree: &sv_parser::SyntaxTree,
    node: &sv_parser::HierarchicalIdentifier,
) -> String {
    syntax_tree
        .get_str(node)
        .map(str::trim)
        .unwrap_or_default()
        .to_string()
}

fn extract_continuous_assign_dataflow(
    syntax_tree: &sv_parser::SyntaxTree,
    node: RefNode,
) -> Vec<DataflowEntry> {
    match node {
        RefNode::ContinuousAssign(assign) => match assign {
            sv_parser::ContinuousAssign::Net(net) => net
                .nodes
                .3
                .nodes
                .0
                .contents()
                .into_iter()
                .filter_map(|assignment| net_assignment_entry(syntax_tree, assignment))
                .collect(),
            sv_parser::ContinuousAssign::Variable(variable) => variable
                .nodes
                .2
                .nodes
                .0
                .contents()
                .into_iter()
                .filter_map(|assignment| variable_assignment_entry(syntax_tree, assignment, &[]))
                .collect(),
        },
        _ => Vec::new(),
    }
}

fn extract_always_dataflow(
    syntax_tree: &sv_parser::SyntaxTree,
    always_construct: &sv_parser::AlwaysConstruct,
) -> Vec<DataflowEntry> {
    let mut dataflow = Vec::new();
    let mut conditions = Vec::new();
    collect_statement_dataflow(
        syntax_tree,
        &always_construct.nodes.1,
        &mut conditions,
        &mut dataflow,
    );
    dataflow
}

fn collect_statement_or_null_dataflow(
    syntax_tree: &sv_parser::SyntaxTree,
    statement: &sv_parser::StatementOrNull,
    conditions: &mut Vec<HashSet<String>>,
    dataflow: &mut Vec<DataflowEntry>,
) {
    if let sv_parser::StatementOrNull::Statement(statement) = statement {
        collect_statement_dataflow(syntax_tree, statement, conditions, dataflow);
    }
}

fn collect_statement_dataflow(
    syntax_tree: &sv_parser::SyntaxTree,
    statement: &sv_parser::Statement,
    conditions: &mut Vec<HashSet<String>>,
    dataflow: &mut Vec<DataflowEntry>,
) {
    match &statement.nodes.2 {
        sv_parser::StatementItem::BlockingAssignment(assignment) => {
            if let Some(entry) = blocking_assignment_entry(syntax_tree, &assignment.0, conditions) {
                dataflow.push(entry);
            }
        }
        sv_parser::StatementItem::NonblockingAssignment(assignment) => {
            if let Some(entry) =
                nonblocking_assignment_entry(syntax_tree, &assignment.0, conditions)
            {
                dataflow.push(entry);
            }
        }
        sv_parser::StatementItem::ConditionalStatement(conditional) => {
            collect_conditional_dataflow(syntax_tree, conditional, conditions, dataflow);
        }
        sv_parser::StatementItem::CaseStatement(case_statement) => {
            collect_case_dataflow(syntax_tree, case_statement, conditions, dataflow);
        }
        sv_parser::StatementItem::SeqBlock(block) => {
            for statement in &block.nodes.3 {
                collect_statement_or_null_dataflow(syntax_tree, statement, conditions, dataflow);
            }
        }
        sv_parser::StatementItem::ProceduralTimingControlStatement(statement) => {
            collect_statement_or_null_dataflow(
                syntax_tree,
                &statement.nodes.1,
                conditions,
                dataflow,
            );
        }
        _ => {}
    }
}

fn collect_conditional_dataflow(
    syntax_tree: &sv_parser::SyntaxTree,
    conditional: &sv_parser::ConditionalStatement,
    conditions: &mut Vec<HashSet<String>>,
    dataflow: &mut Vec<DataflowEntry>,
) {
    let predicate = cond_predicate_signals(syntax_tree, &conditional.nodes.2.nodes.1);

    conditions.push(predicate.clone());
    collect_statement_or_null_dataflow(syntax_tree, &conditional.nodes.3, conditions, dataflow);
    conditions.pop();

    let mut prior_else_if_predicates: Vec<HashSet<String>> = Vec::new();

    for (_, _, predicate_group, statement) in &conditional.nodes.4 {
        conditions.push(predicate.clone());
        for prior_predicate in &prior_else_if_predicates {
            conditions.push(prior_predicate.clone());
        }

        let current_predicate = cond_predicate_signals(syntax_tree, &predicate_group.nodes.1);
        conditions.push(current_predicate.clone());
        collect_statement_or_null_dataflow(syntax_tree, statement, conditions, dataflow);
        conditions.pop();
        for _ in &prior_else_if_predicates {
            conditions.pop();
        }
        conditions.pop();

        prior_else_if_predicates.push(current_predicate);
    }

    if let Some((_, statement)) = &conditional.nodes.5 {
        conditions.push(predicate.clone());
        for prior_predicate in &prior_else_if_predicates {
            conditions.push(prior_predicate.clone());
        }
        collect_statement_or_null_dataflow(syntax_tree, statement, conditions, dataflow);
        for _ in &prior_else_if_predicates {
            conditions.pop();
        }
        conditions.pop();
    }
}

fn collect_case_dataflow(
    syntax_tree: &sv_parser::SyntaxTree,
    case_statement: &sv_parser::CaseStatement,
    conditions: &mut Vec<HashSet<String>>,
    dataflow: &mut Vec<DataflowEntry>,
) {
    if let sv_parser::CaseStatement::Normal(case_statement) = case_statement {
        let selector = expression_signals(syntax_tree, &case_statement.nodes.2.nodes.1.nodes.0);

        collect_case_item_dataflow(
            syntax_tree,
            &case_statement.nodes.3,
            &selector,
            conditions,
            dataflow,
        );
        for item in &case_statement.nodes.4 {
            collect_case_item_dataflow(syntax_tree, item, &selector, conditions, dataflow);
        }
    }
}

fn collect_case_item_dataflow(
    syntax_tree: &sv_parser::SyntaxTree,
    item: &sv_parser::CaseItem,
    selector: &HashSet<String>,
    conditions: &mut Vec<HashSet<String>>,
    dataflow: &mut Vec<DataflowEntry>,
) {
    match item {
        sv_parser::CaseItem::NonDefault(item) => {
            conditions.push(selector.clone());
            collect_statement_or_null_dataflow(syntax_tree, &item.nodes.2, conditions, dataflow);
            conditions.pop();
        }
        sv_parser::CaseItem::Default(item) => {
            conditions.push(selector.clone());
            collect_statement_or_null_dataflow(syntax_tree, &item.nodes.2, conditions, dataflow);
            conditions.pop();
        }
    }
}

fn cond_predicate_signals(
    syntax_tree: &sv_parser::SyntaxTree,
    predicate: &sv_parser::CondPredicate,
) -> HashSet<String> {
    let mut signals = HashSet::new();
    for item in predicate.nodes.0.contents() {
        match item {
            sv_parser::ExpressionOrCondPattern::Expression(expr) => {
                signals.extend(expression_signals(syntax_tree, expr));
            }
            sv_parser::ExpressionOrCondPattern::CondPattern(pattern) => {
                signals.extend(expression_signals(syntax_tree, &pattern.nodes.0));
            }
        }
    }
    signals
}

fn blocking_assignment_entry(
    syntax_tree: &sv_parser::SyntaxTree,
    assignment: &sv_parser::BlockingAssignment,
    conditions: &[HashSet<String>],
) -> Option<DataflowEntry> {
    match assignment {
        sv_parser::BlockingAssignment::Variable(assign) => entry_from_variable_lvalue_and_expr(
            syntax_tree,
            &assign.nodes.0,
            &assign.nodes.3,
            conditions,
        ),
        sv_parser::BlockingAssignment::OperatorAssignment(assign)
            if syntax_tree.get_str(&assign.nodes.1).map(str::trim) == Some("=") =>
        {
            entry_from_variable_lvalue_and_expr(
                syntax_tree,
                &assign.nodes.0,
                &assign.nodes.2,
                conditions,
            )
        }
        sv_parser::BlockingAssignment::OperatorAssignment(_) => None,
        _ => None,
    }
}

fn nonblocking_assignment_entry(
    syntax_tree: &sv_parser::SyntaxTree,
    assignment: &sv_parser::NonblockingAssignment,
    conditions: &[HashSet<String>],
) -> Option<DataflowEntry> {
    entry_from_variable_lvalue_and_expr(
        syntax_tree,
        &assignment.nodes.0,
        &assignment.nodes.3,
        conditions,
    )
}

fn variable_assignment_entry(
    syntax_tree: &sv_parser::SyntaxTree,
    assignment: &sv_parser::VariableAssignment,
    conditions: &[HashSet<String>],
) -> Option<DataflowEntry> {
    entry_from_variable_lvalue_and_expr(
        syntax_tree,
        &assignment.nodes.0,
        &assignment.nodes.2,
        conditions,
    )
}

fn net_assignment_entry(
    syntax_tree: &sv_parser::SyntaxTree,
    assignment: &sv_parser::NetAssignment,
) -> Option<DataflowEntry> {
    entry_from_net_lvalue_and_expr(syntax_tree, &assignment.nodes.0, &assignment.nodes.2, &[])
}

fn entry_from_variable_lvalue_and_expr(
    syntax_tree: &sv_parser::SyntaxTree,
    lvalue: &sv_parser::VariableLvalue,
    expression: &sv_parser::Expression,
    conditions: &[HashSet<String>],
) -> Option<DataflowEntry> {
    let output = variable_lvalue_name(syntax_tree, lvalue)?;
    let mut inputs = expression_signals(syntax_tree, expression);
    extend_condition_inputs(&mut inputs, conditions);
    Some(to_entry(output, inputs))
}

fn entry_from_net_lvalue_and_expr(
    syntax_tree: &sv_parser::SyntaxTree,
    lvalue: &sv_parser::NetLvalue,
    expression: &sv_parser::Expression,
    conditions: &[HashSet<String>],
) -> Option<DataflowEntry> {
    let output = net_lvalue_name(syntax_tree, lvalue)?;
    let mut inputs = expression_signals(syntax_tree, expression);
    extend_condition_inputs(&mut inputs, conditions);
    Some(to_entry(output, inputs))
}

fn extend_condition_inputs(inputs: &mut HashSet<String>, conditions: &[HashSet<String>]) {
    for condition in conditions {
        inputs.extend(condition.iter().cloned());
    }
}

fn variable_lvalue_name(
    syntax_tree: &sv_parser::SyntaxTree,
    lvalue: &sv_parser::VariableLvalue,
) -> Option<String> {
    match lvalue {
        sv_parser::VariableLvalue::Identifier(identifier) => {
            Some(identifier_text(syntax_tree, (&identifier.nodes.1).into()))
        }
        _ => None,
    }
}

fn net_lvalue_name(
    syntax_tree: &sv_parser::SyntaxTree,
    lvalue: &sv_parser::NetLvalue,
) -> Option<String> {
    match lvalue {
        sv_parser::NetLvalue::Identifier(identifier) => {
            Some(identifier_text(syntax_tree, (&identifier.nodes.0).into()))
        }
        _ => None,
    }
}

fn expression_signals(
    syntax_tree: &sv_parser::SyntaxTree,
    expression: &sv_parser::Expression,
) -> HashSet<String> {
    signals_from_node_text(syntax_tree, expression.into())
}

fn signals_from_node_text(syntax_tree: &sv_parser::SyntaxTree, node: RefNode) -> HashSet<String> {
    let mut names = HashSet::new();

    for child in node.into_iter() {
        match child {
            RefNode::PrimaryHierarchical(primary) => {
                names.insert(hierarchical_identifier_text(syntax_tree, &primary.nodes.1));
            }
            RefNode::HierarchicalVariableIdentifier(id) => {
                names.insert(hierarchical_variable_identifier_text(syntax_tree, id));
            }
            RefNode::PsOrHierarchicalNetIdentifier(id) => {
                names.insert(ps_or_hierarchical_net_identifier_text(syntax_tree, id));
            }
            RefNode::PortIdentifier(id) => {
                names.insert(identifier_text(syntax_tree, id.into()));
            }
            _ => {}
        }
    }

    names.retain(|name| is_signal_name(name));
    names
}

fn is_signal_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }

    let reserved = [
        "assign",
        "always_comb",
        "always_ff",
        "begin",
        "end",
        "if",
        "else",
        "case",
        "endcase",
        "module",
        "endmodule",
        "logic",
        "input",
        "output",
        "posedge",
        "default",
    ];

    if reserved.contains(&name) {
        return false;
    }

    if name.chars().all(|ch| ch.is_ascii_digit()) {
        return false;
    }

    name.chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
}

fn to_entry(output: String, inputs: HashSet<String>) -> DataflowEntry {
    DataflowEntry {
        output: crate::types::SignalId(output),
        inputs: inputs.into_iter().map(crate::types::SignalId).collect(),
    }
}

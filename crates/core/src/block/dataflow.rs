#![allow(dead_code)]

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use sv_parser::{unwrap_locate, unwrap_node, RefNode};

use super::{Block, BlockSet, BlockType, CircuitType, DataflowEntry};
use crate::ast::ParsedFile;
use crate::types::{BlockId, SignalNode};

#[derive(Debug, Clone)]
struct ModuleTemplate {
    blocks: Vec<Block>,
    port_order: Vec<String>,
}

#[derive(Debug, Clone)]
struct ModuleInstance {
    module_name: String,
    instance_name: String,
    line_start: usize,
    connections: HashMap<String, HashSet<SignalNode>>,
}

pub fn elaborate_block_set(
    files: &[ParsedFile],
    template_block_set: &BlockSet,
) -> Result<BlockSet> {
    let mut templates = module_templates_from_block_set(template_block_set);
    add_empty_module_templates(files, &mut templates);
    if templates.is_empty() {
        return BlockSet::new(Vec::new());
    }

    let instances_by_module = collect_module_instances(files, &templates);
    let instantiated_modules = instances_by_module
        .values()
        .flat_map(|instances| {
            instances
                .iter()
                .map(|instance| instance.module_name.clone())
        })
        .collect::<HashSet<_>>();

    let mut top_modules = templates
        .keys()
        .filter(|module_name| !instantiated_modules.contains(*module_name))
        .cloned()
        .collect::<Vec<_>>();
    if top_modules.is_empty() {
        top_modules = templates.keys().cloned().collect();
    }
    top_modules.sort();

    let mut blocks = Vec::new();
    let mut next_block_id = 0u64;
    for module_name in top_modules {
        let instance_path = format!("TOP.{module_name}");
        elaborate_module_instance(
            &module_name,
            &instance_path,
            None,
            None,
            &templates,
            &instances_by_module,
            &mut next_block_id,
            &mut blocks,
        )?;
    }

    BlockSet::new(blocks)
}

fn module_templates_from_block_set(block_set: &BlockSet) -> HashMap<String, ModuleTemplate> {
    let mut templates = HashMap::new();

    for block in block_set.blocks() {
        let entry = templates
            .entry(block.module_scope().to_string())
            .or_insert_with(|| ModuleTemplate {
                blocks: Vec::new(),
                port_order: Vec::new(),
            });

        if let Some(port_name) = port_name_from_block(block) {
            entry.port_order.push(port_name);
        }

        entry.blocks.push(block.clone());
    }

    templates
}

fn add_empty_module_templates(
    files: &[ParsedFile],
    templates: &mut HashMap<String, ModuleTemplate>,
) {
    for file in files {
        for node in &file.syntax_tree {
            match node {
                RefNode::ModuleDeclarationAnsi(module) => {
                    if let Some(module_name) = module_name_from_node(
                        &file.syntax_tree,
                        RefNode::ModuleDeclarationAnsi(module),
                    ) {
                        templates
                            .entry(module_name)
                            .or_insert_with(|| ModuleTemplate {
                                blocks: Vec::new(),
                                port_order: Vec::new(),
                            });
                    }
                }
                RefNode::ModuleDeclarationNonansi(module) => {
                    if let Some(module_name) = module_name_from_node(
                        &file.syntax_tree,
                        RefNode::ModuleDeclarationNonansi(module),
                    ) {
                        templates
                            .entry(module_name)
                            .or_insert_with(|| ModuleTemplate {
                                blocks: Vec::new(),
                                port_order: Vec::new(),
                            });
                    }
                }
                _ => {}
            }
        }
    }
}

fn collect_module_instances(
    files: &[ParsedFile],
    templates: &HashMap<String, ModuleTemplate>,
) -> HashMap<String, Vec<ModuleInstance>> {
    let mut instances_by_module = HashMap::new();

    for file in files {
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
                sv_parser::NodeEvent::Enter(RefNode::ModuleInstantiation(module_instantiation)) => {
                    let Some(parent_module) = current_module.as_deref() else {
                        continue;
                    };

                    let module_name =
                        identifier_text(&file.syntax_tree, (&module_instantiation.nodes.0).into());
                    let port_order = templates
                        .get(&module_name)
                        .map(|template| template.port_order.as_slice())
                        .unwrap_or(&[]);
                    let line_start = start_line(RefNode::ModuleInstantiation(module_instantiation));

                    for hierarchical_instance in module_instantiation.nodes.2.contents() {
                        instances_by_module
                            .entry(parent_module.to_string())
                            .or_insert_with(Vec::new)
                            .push(ModuleInstance {
                                module_name: module_name.clone(),
                                instance_name: identifier_text(
                                    &file.syntax_tree,
                                    (&hierarchical_instance.nodes.0.nodes.0).into(),
                                ),
                                line_start,
                                connections: module_instance_connections(
                                    &file.syntax_tree,
                                    hierarchical_instance,
                                    port_order,
                                ),
                            });
                    }
                }
                _ => {}
            }
        }
    }

    for instances in instances_by_module.values_mut() {
        instances.sort_by(|left, right| {
            left.instance_name
                .cmp(&right.instance_name)
                .then_with(|| left.module_name.cmp(&right.module_name))
                .then_with(|| left.line_start.cmp(&right.line_start))
        });
    }

    instances_by_module
}

fn module_instance_connections(
    syntax_tree: &sv_parser::SyntaxTree,
    hierarchical_instance: &sv_parser::HierarchicalInstance,
    port_order: &[String],
) -> HashMap<String, HashSet<SignalNode>> {
    let Some(port_connections) = hierarchical_instance.nodes.1.nodes.1.as_ref() else {
        return HashMap::new();
    };

    match port_connections {
        sv_parser::ListOfPortConnections::Named(named) => {
            named_port_connections(syntax_tree, &named.nodes.0, port_order)
        }
        sv_parser::ListOfPortConnections::Ordered(ordered) => {
            ordered_port_connections(syntax_tree, &ordered.nodes.0, port_order)
        }
    }
}

fn named_port_connections(
    syntax_tree: &sv_parser::SyntaxTree,
    connections: &sv_parser::List<sv_parser::Symbol, sv_parser::NamedPortConnection>,
    port_order: &[String],
) -> HashMap<String, HashSet<SignalNode>> {
    let mut resolved = HashMap::new();
    let mut wildcard = false;

    for connection in connections.contents() {
        match connection {
            sv_parser::NamedPortConnection::Identifier(connection) => {
                let port_name = identifier_text(syntax_tree, (&connection.nodes.2).into());
                let signals = connection
                    .nodes
                    .3
                    .as_ref()
                    .and_then(|expression| expression.nodes.1.as_ref())
                    .map(|expression| expression_signal_nodes(syntax_tree, expression))
                    .unwrap_or_else(|| HashSet::from([SignalNode::named(port_name.clone())]));
                resolved.insert(port_name, signals);
            }
            sv_parser::NamedPortConnection::Asterisk(_) => {
                wildcard = true;
            }
        }
    }

    if wildcard {
        for port_name in port_order {
            resolved
                .entry(port_name.clone())
                .or_insert_with(|| HashSet::from([SignalNode::named(port_name.clone())]));
        }
    }

    resolved
}

fn ordered_port_connections(
    syntax_tree: &sv_parser::SyntaxTree,
    connections: &sv_parser::List<sv_parser::Symbol, sv_parser::OrderedPortConnection>,
    port_order: &[String],
) -> HashMap<String, HashSet<SignalNode>> {
    let mut resolved = HashMap::new();

    for (index, connection) in connections.contents().into_iter().enumerate() {
        let Some(port_name) = port_order.get(index) else {
            continue;
        };

        let signals = connection
            .nodes
            .1
            .as_ref()
            .map(|expression| expression_signal_nodes(syntax_tree, expression))
            .unwrap_or_default();
        resolved.insert(port_name.clone(), signals);
    }

    resolved
}

#[allow(clippy::too_many_arguments)]
fn elaborate_module_instance(
    module_name: &str,
    instance_path: &str,
    parent_path: Option<&str>,
    instance: Option<&ModuleInstance>,
    templates: &HashMap<String, ModuleTemplate>,
    instances_by_module: &HashMap<String, Vec<ModuleInstance>>,
    next_block_id: &mut u64,
    blocks: &mut Vec<Block>,
) -> Result<()> {
    let Some(template) = templates.get(module_name) else {
        return Ok(());
    };

    for block in &template.blocks {
        if matches!(
            block.block_type(),
            BlockType::ModInput | BlockType::ModOutput
        ) {
            continue;
        }

        blocks.push(qualify_block(block, instance_path, next_block_id)?);
    }

    match (parent_path, instance) {
        (Some(parent_scope), Some(instance)) => {
            for block in &template.blocks {
                if !matches!(
                    block.block_type(),
                    BlockType::ModInput | BlockType::ModOutput
                ) {
                    continue;
                }

                if let Some(bridge) =
                    bridge_port_block(block, parent_scope, instance_path, instance, next_block_id)?
                {
                    blocks.push(bridge);
                }
            }
        }
        _ => {
            for block in &template.blocks {
                if !matches!(
                    block.block_type(),
                    BlockType::ModInput | BlockType::ModOutput
                ) {
                    continue;
                }

                blocks.push(qualify_block(block, instance_path, next_block_id)?);
            }
        }
    }

    if let Some(instances) = instances_by_module.get(module_name) {
        for child_instance in instances {
            let child_path = format!("{instance_path}.{}", child_instance.instance_name);
            elaborate_module_instance(
                &child_instance.module_name,
                &child_path,
                Some(instance_path),
                Some(child_instance),
                templates,
                instances_by_module,
                next_block_id,
                blocks,
            )?;
        }
    }

    Ok(())
}

fn qualify_block(block: &Block, scope: &str, next_block_id: &mut u64) -> Result<Block> {
    let dataflow = block
        .dataflow()
        .iter()
        .map(|entry| DataflowEntry {
            output: entry
                .output
                .iter()
                .map(|signal| qualify_signal(signal, scope))
                .collect(),
            inputs: entry
                .inputs
                .iter()
                .map(|signal| qualify_signal(signal, scope))
                .collect(),
        })
        .collect();

    Block::new(
        take_block_id(next_block_id),
        block.block_type(),
        block.circuit_type(),
        scope,
        block.source_file(),
        block.line_start(),
        block.line_end(),
        dataflow,
        block.code_snippet(),
    )
}

fn bridge_port_block(
    template_port_block: &Block,
    parent_scope: &str,
    child_scope: &str,
    instance: &ModuleInstance,
    next_block_id: &mut u64,
) -> Result<Option<Block>> {
    let Some(port_name) = port_name_from_block(template_port_block) else {
        return Ok(None);
    };

    let connected = instance
        .connections
        .get(&port_name)
        .cloned()
        .unwrap_or_default();
    let child_port = qualify_signal(&SignalNode::named(port_name), child_scope);
    let parent_signals = connected
        .iter()
        .map(|signal| qualify_signal(signal, parent_scope))
        .collect::<HashSet<_>>();

    let dataflow = match template_port_block.block_type() {
        BlockType::ModInput => vec![DataflowEntry {
            output: vec![child_port],
            inputs: parent_signals,
        }],
        BlockType::ModOutput => {
            let mut output = parent_signals
                .iter()
                .filter(|signal| signal.is_variable())
                .cloned()
                .collect::<Vec<_>>();
            output.sort_by(|left, right| left.name.cmp(&right.name));
            vec![DataflowEntry {
                output,
                inputs: HashSet::from([child_port]),
            }]
        }
        _ => return Ok(None),
    };

    Ok(Some(Block::new(
        take_block_id(next_block_id),
        template_port_block.block_type(),
        CircuitType::Combinational,
        child_scope,
        template_port_block.source_file(),
        template_port_block.line_start(),
        template_port_block.line_end(),
        dataflow,
        template_port_block.code_snippet(),
    )?))
}

fn port_name_from_block(block: &Block) -> Option<String> {
    let entry = block.dataflow().first()?;

    match block.block_type() {
        BlockType::ModInput => entry.output.first().map(|signal| signal.name.clone()),
        BlockType::ModOutput => entry.inputs.iter().next().map(|signal| signal.name.clone()),
        _ => None,
    }
}

fn qualify_signal(signal: &SignalNode, scope: &str) -> SignalNode {
    if signal.is_literal() || signal.name.starts_with("TOP.") {
        return signal.clone();
    }

    SignalNode::variable(format!("{scope}.{}", signal.name), signal.locate)
}

fn take_block_id(next_block_id: &mut u64) -> BlockId {
    let block_id = BlockId(*next_block_id);
    *next_block_id += 1;
    block_id
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

fn locate_from_node(node: RefNode) -> crate::types::SignalLocate {
    unwrap_locate!(node)
        .map(|loc| crate::types::SignalLocate {
            offset: loc.offset,
            line: usize::try_from(loc.line).unwrap_or(0),
            len: loc.len,
        })
        .unwrap_or_else(|| crate::types::SignalLocate::unknown(0))
}

fn identifier_signal_node(
    syntax_tree: &sv_parser::SyntaxTree,
    node: RefNode,
) -> crate::types::SignalNode {
    let name = identifier_text(syntax_tree, node.clone());
    crate::types::SignalNode::variable(name, locate_from_node(node))
}

fn hierarchical_variable_identifier_signal_node(
    syntax_tree: &sv_parser::SyntaxTree,
    node: &sv_parser::HierarchicalVariableIdentifier,
) -> crate::types::SignalNode {
    crate::types::SignalNode::variable(
        hierarchical_variable_identifier_text(syntax_tree, node),
        locate_from_node(node.into()),
    )
}

fn ps_or_hierarchical_net_identifier_signal_node(
    syntax_tree: &sv_parser::SyntaxTree,
    node: &sv_parser::PsOrHierarchicalNetIdentifier,
) -> crate::types::SignalNode {
    crate::types::SignalNode::variable(
        ps_or_hierarchical_net_identifier_text(syntax_tree, node),
        locate_from_node(node.into()),
    )
}

fn hierarchical_identifier_signal_node(
    syntax_tree: &sv_parser::SyntaxTree,
    node: &sv_parser::HierarchicalIdentifier,
) -> crate::types::SignalNode {
    crate::types::SignalNode::variable(
        hierarchical_identifier_text(syntax_tree, node),
        locate_from_node(node.into()),
    )
}

fn snippet_from_source(source_text: &str, line_start: usize, line_end: usize) -> String {
    source_text
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let line_no = index + 1;
            (line_start <= line_no && line_no <= line_end).then_some(line)
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn start_line(node: RefNode) -> usize {
    unwrap_locate!(node)
        .and_then(|loc| usize::try_from(loc.line).ok())
        .unwrap_or(1)
}

fn locate_line(loc: &sv_parser::Locate) -> Option<usize> {
    usize::try_from(loc.line).ok()
}

fn keyword_line(keyword: &sv_parser::Keyword) -> Option<usize> {
    locate_line(&keyword.nodes.0)
}

fn symbol_line(symbol: &sv_parser::Symbol) -> Option<usize> {
    locate_line(&symbol.nodes.0)
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
        _ => unwrap_locate!(statement).and_then(locate_line),
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
        _ => unwrap_locate!(statement).and_then(locate_line),
    }
}

fn continuous_assign_end_line(assign: &sv_parser::ContinuousAssign) -> Option<usize> {
    match assign {
        sv_parser::ContinuousAssign::Net(assign) => symbol_line(&assign.nodes.4),
        sv_parser::ContinuousAssign::Variable(assign) => symbol_line(&assign.nodes.3),
    }
}

fn always_end_line(always_construct: &sv_parser::AlwaysConstruct) -> Option<usize> {
    statement_end_line(&always_construct.nodes.1)
}

fn port_block_details(syntax_tree: &sv_parser::SyntaxTree, signal_name: &str) -> Option<usize> {
    for node in syntax_tree {
        match node {
            RefNode::AnsiPortDeclaration(port)
                if ansi_port_block_data(syntax_tree, port)
                    .is_some_and(|(_, name)| name == signal_name) =>
            {
                return Some(start_line(RefNode::AnsiPortDeclaration(port)));
            }
            RefNode::InputDeclaration(port)
                if nonansi_input_port_block_data(syntax_tree, port)
                    .is_some_and(|ports| ports.iter().any(|(_, name)| name == signal_name)) =>
            {
                return Some(start_line(RefNode::InputDeclaration(port)));
            }
            RefNode::OutputDeclaration(port)
                if nonansi_output_port_block_data(syntax_tree, port)
                    .is_some_and(|ports| ports.iter().any(|(_, name)| name == signal_name)) =>
            {
                return Some(start_line(RefNode::OutputDeclaration(port)));
            }
            _ => {}
        }
    }

    None
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
    conditions: &mut Vec<HashSet<crate::types::SignalNode>>,
    dataflow: &mut Vec<DataflowEntry>,
) {
    if let sv_parser::StatementOrNull::Statement(statement) = statement {
        collect_statement_dataflow(syntax_tree, statement, conditions, dataflow);
    }
}

fn collect_statement_dataflow(
    syntax_tree: &sv_parser::SyntaxTree,
    statement: &sv_parser::Statement,
    conditions: &mut Vec<HashSet<crate::types::SignalNode>>,
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
            for declaration in &block.nodes.2 {
                collect_block_item_declaration_dataflow(
                    syntax_tree,
                    declaration,
                    conditions,
                    dataflow,
                );
            }
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

fn collect_block_item_declaration_dataflow(
    syntax_tree: &sv_parser::SyntaxTree,
    declaration: &sv_parser::BlockItemDeclaration,
    conditions: &[HashSet<crate::types::SignalNode>],
    dataflow: &mut Vec<DataflowEntry>,
) {
    let sv_parser::BlockItemDeclaration::Data(declaration) = declaration else {
        return;
    };

    let sv_parser::DataDeclaration::Variable(declaration) = &declaration.nodes.1 else {
        return;
    };

    for assignment in declaration.nodes.4.nodes.0.contents() {
        if let Some(entry) = variable_decl_assignment_entry(syntax_tree, assignment, conditions) {
            dataflow.push(entry);
        }
    }
}

fn collect_conditional_dataflow(
    syntax_tree: &sv_parser::SyntaxTree,
    conditional: &sv_parser::ConditionalStatement,
    conditions: &mut Vec<HashSet<crate::types::SignalNode>>,
    dataflow: &mut Vec<DataflowEntry>,
) {
    let predicate = cond_predicate_signals(syntax_tree, &conditional.nodes.2.nodes.1);

    conditions.push(predicate.clone());
    collect_statement_or_null_dataflow(syntax_tree, &conditional.nodes.3, conditions, dataflow);
    conditions.pop();

    let mut prior_else_if_predicates: Vec<HashSet<crate::types::SignalNode>> = Vec::new();

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
    conditions: &mut Vec<HashSet<crate::types::SignalNode>>,
    dataflow: &mut Vec<DataflowEntry>,
) {
    if let sv_parser::CaseStatement::Normal(case_statement) = case_statement {
        let selector =
            expression_signal_nodes(syntax_tree, &case_statement.nodes.2.nodes.1.nodes.0);

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
    selector: &HashSet<crate::types::SignalNode>,
    conditions: &mut Vec<HashSet<crate::types::SignalNode>>,
    dataflow: &mut Vec<DataflowEntry>,
) {
    match item {
        sv_parser::CaseItem::NonDefault(item) => {
            let mut case_condition = selector.clone();
            for expression in item.nodes.0.contents() {
                case_condition.extend(expression_signal_nodes(syntax_tree, &expression.nodes.0));
            }
            conditions.push(case_condition);
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
) -> HashSet<crate::types::SignalNode> {
    let mut signals = HashSet::new();
    for item in predicate.nodes.0.contents() {
        match item {
            sv_parser::ExpressionOrCondPattern::Expression(expr) => {
                signals.extend(expression_signal_nodes(syntax_tree, expr));
            }
            sv_parser::ExpressionOrCondPattern::CondPattern(pattern) => {
                signals.extend(expression_signal_nodes(syntax_tree, &pattern.nodes.0));
            }
        }
    }
    signals
}

fn blocking_assignment_entry(
    syntax_tree: &sv_parser::SyntaxTree,
    assignment: &sv_parser::BlockingAssignment,
    conditions: &[HashSet<crate::types::SignalNode>],
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
    conditions: &[HashSet<crate::types::SignalNode>],
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
    conditions: &[HashSet<crate::types::SignalNode>],
) -> Option<DataflowEntry> {
    entry_from_variable_lvalue_and_expr(
        syntax_tree,
        &assignment.nodes.0,
        &assignment.nodes.2,
        conditions,
    )
}

fn variable_decl_assignment_entry(
    syntax_tree: &sv_parser::SyntaxTree,
    assignment: &sv_parser::VariableDeclAssignment,
    conditions: &[HashSet<crate::types::SignalNode>],
) -> Option<DataflowEntry> {
    let sv_parser::VariableDeclAssignment::Variable(assignment) = assignment else {
        return None;
    };
    let (_, expression) = assignment.nodes.2.as_ref()?;

    let output = vec![identifier_signal_node(
        syntax_tree,
        (&assignment.nodes.0).into(),
    )];
    let mut inputs = expression_signal_nodes(syntax_tree, expression);
    extend_condition_inputs(&mut inputs, conditions);
    Some(DataflowEntry { output, inputs })
}

fn net_assignment_entry(
    syntax_tree: &sv_parser::SyntaxTree,
    assignment: &sv_parser::NetAssignment,
) -> Option<DataflowEntry> {
    entry_from_net_lvalue_and_expr(syntax_tree, &assignment.nodes.0, &assignment.nodes.2, &[])
}

#[allow(clippy::too_many_arguments)]
fn entry_from_variable_lvalue_and_expr(
    syntax_tree: &sv_parser::SyntaxTree,
    lvalue: &sv_parser::VariableLvalue,
    expression: &sv_parser::Expression,
    conditions: &[HashSet<crate::types::SignalNode>],
) -> Option<DataflowEntry> {
    let output = variable_lvalue_nodes(syntax_tree, lvalue);
    if output.is_empty() {
        return None;
    }
    let mut inputs = expression_signal_nodes(syntax_tree, expression);
    extend_condition_inputs(&mut inputs, conditions);
    Some(DataflowEntry { output, inputs })
}

fn entry_from_net_lvalue_and_expr(
    syntax_tree: &sv_parser::SyntaxTree,
    lvalue: &sv_parser::NetLvalue,
    expression: &sv_parser::Expression,
    conditions: &[HashSet<crate::types::SignalNode>],
) -> Option<DataflowEntry> {
    let output = net_lvalue_nodes(syntax_tree, lvalue);
    if output.is_empty() {
        return None;
    }
    let mut inputs = expression_signal_nodes(syntax_tree, expression);
    extend_condition_inputs(&mut inputs, conditions);
    Some(DataflowEntry { output, inputs })
}

fn extend_condition_inputs(
    inputs: &mut HashSet<crate::types::SignalNode>,
    conditions: &[HashSet<crate::types::SignalNode>],
) {
    for condition in conditions {
        inputs.extend(condition.iter().cloned());
    }
}

fn variable_lvalue_nodes(
    syntax_tree: &sv_parser::SyntaxTree,
    lvalue: &sv_parser::VariableLvalue,
) -> Vec<crate::types::SignalNode> {
    match lvalue {
        sv_parser::VariableLvalue::Identifier(identifier) => {
            vec![hierarchical_variable_identifier_signal_node(
                syntax_tree,
                &identifier.nodes.1,
            )]
        }
        sv_parser::VariableLvalue::Lvalue(lvalues) => lvalues
            .nodes
            .0
            .nodes
            .1
            .contents()
            .into_iter()
            .flat_map(|child| variable_lvalue_nodes(syntax_tree, child))
            .collect(),
        _ => Vec::new(),
    }
}

fn net_lvalue_nodes(
    syntax_tree: &sv_parser::SyntaxTree,
    lvalue: &sv_parser::NetLvalue,
) -> Vec<crate::types::SignalNode> {
    match lvalue {
        sv_parser::NetLvalue::Identifier(identifier) => {
            vec![ps_or_hierarchical_net_identifier_signal_node(
                syntax_tree,
                &identifier.nodes.0,
            )]
        }
        sv_parser::NetLvalue::Lvalue(lvalues) => lvalues
            .nodes
            .0
            .nodes
            .1
            .contents()
            .into_iter()
            .flat_map(|child| net_lvalue_nodes(syntax_tree, child))
            .collect(),
        _ => Vec::new(),
    }
}

fn expression_signal_nodes(
    syntax_tree: &sv_parser::SyntaxTree,
    expression: &sv_parser::Expression,
) -> HashSet<crate::types::SignalNode> {
    signal_nodes_from_node_text(syntax_tree, expression.into())
}

fn signal_nodes_from_node_text(
    syntax_tree: &sv_parser::SyntaxTree,
    node: RefNode,
) -> HashSet<crate::types::SignalNode> {
    let mut names = HashSet::new();

    for child in node.into_iter() {
        match child {
            RefNode::PrimaryHierarchical(primary) => {
                names.insert(hierarchical_identifier_signal_node(
                    syntax_tree,
                    &primary.nodes.1,
                ));
            }
            RefNode::HierarchicalVariableIdentifier(id) => {
                names.insert(hierarchical_variable_identifier_signal_node(
                    syntax_tree,
                    id,
                ));
            }
            RefNode::PsOrHierarchicalNetIdentifier(id) => {
                names.insert(ps_or_hierarchical_net_identifier_signal_node(
                    syntax_tree,
                    id,
                ));
            }
            RefNode::PortIdentifier(id) => {
                names.insert(identifier_signal_node(syntax_tree, id.into()));
            }
            RefNode::Number(number) => {
                let text = syntax_tree
                    .get_str(number)
                    .map(str::trim)
                    .unwrap_or_default()
                    .to_string();
                names.insert(crate::types::SignalNode::literal_with_locate(
                    text,
                    locate_from_node(number.into()),
                ));
            }
            RefNode::PrimaryLiteral(literal) => {
                let text = syntax_tree
                    .get_str(literal)
                    .map(str::trim)
                    .unwrap_or_default()
                    .to_string();
                names.insert(crate::types::SignalNode::literal_with_locate(
                    text,
                    locate_from_node(literal.into()),
                ));
            }
            RefNode::TimeLiteral(literal) => {
                let text = syntax_tree
                    .get_str(literal)
                    .map(str::trim)
                    .unwrap_or_default()
                    .to_string();
                names.insert(crate::types::SignalNode::literal_with_locate(
                    text,
                    locate_from_node(literal.into()),
                ));
            }
            RefNode::StringLiteral(literal) => {
                let text = syntax_tree
                    .get_str(literal)
                    .map(str::trim)
                    .unwrap_or_default()
                    .to_string();
                names.insert(crate::types::SignalNode::literal_with_locate(
                    text,
                    locate_from_node(literal.into()),
                ));
            }
            RefNode::UnbasedUnsizedLiteral(literal) => {
                let text = syntax_tree
                    .get_str(literal)
                    .map(str::trim)
                    .unwrap_or_default()
                    .to_string();
                names.insert(crate::types::SignalNode::literal_with_locate(
                    text,
                    locate_from_node(literal.into()),
                ));
            }
            _ => {}
        }
    }

    names.retain(|signal| signal.is_literal() || is_signal_name(signal.as_str()));
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

fn ansi_port_block_data(
    syntax_tree: &sv_parser::SyntaxTree,
    port: &sv_parser::AnsiPortDeclaration,
) -> Option<(BlockType, String)> {
    match port {
        sv_parser::AnsiPortDeclaration::Net(port) => {
            let direction = match port.nodes.0.as_ref()? {
                sv_parser::NetPortHeaderOrInterfacePortHeader::NetPortHeader(header) => {
                    header.nodes.0.as_ref()?
                }
                sv_parser::NetPortHeaderOrInterfacePortHeader::InterfacePortHeader(_) => {
                    return None;
                }
            };

            Some((
                block_type_for_direction(direction)?,
                identifier_text(syntax_tree, (&port.nodes.1).into()),
            ))
        }
        sv_parser::AnsiPortDeclaration::Variable(port) => Some((
            block_type_for_direction(port.nodes.0.as_ref()?.nodes.0.as_ref()?)?,
            identifier_text(syntax_tree, (&port.nodes.1).into()),
        )),
        sv_parser::AnsiPortDeclaration::Paren(port) => Some((
            block_type_for_direction(port.nodes.0.as_ref()?)?,
            identifier_text(syntax_tree, (&port.nodes.2).into()),
        )),
    }
}

fn block_type_for_direction(direction: &sv_parser::PortDirection) -> Option<BlockType> {
    match direction {
        sv_parser::PortDirection::Input(_) => Some(BlockType::ModInput),
        sv_parser::PortDirection::Output(_) => Some(BlockType::ModOutput),
        _ => None,
    }
}

fn nonansi_port_block_data(
    syntax_tree: &sv_parser::SyntaxTree,
    port_declaration: &sv_parser::PortDeclaration,
) -> Option<Vec<(BlockType, String)>> {
    match port_declaration {
        sv_parser::PortDeclaration::Input(port) => {
            nonansi_input_port_block_data(syntax_tree, &port.nodes.1)
        }
        sv_parser::PortDeclaration::Output(port) => {
            nonansi_output_port_block_data(syntax_tree, &port.nodes.1)
        }
        _ => None,
    }
}

fn nonansi_input_port_block_data(
    syntax_tree: &sv_parser::SyntaxTree,
    declaration: &sv_parser::InputDeclaration,
) -> Option<Vec<(BlockType, String)>> {
    match declaration {
        sv_parser::InputDeclaration::Net(declaration) => Some(
            declaration
                .nodes
                .2
                .nodes
                .0
                .contents()
                .into_iter()
                .map(|(identifier, _)| {
                    (
                        BlockType::ModInput,
                        identifier_text(syntax_tree, identifier.into()),
                    )
                })
                .collect(),
        ),
        sv_parser::InputDeclaration::Variable(declaration) => Some(
            declaration
                .nodes
                .2
                .nodes
                .0
                .contents()
                .into_iter()
                .map(|(identifier, _)| {
                    (
                        BlockType::ModInput,
                        identifier_text(syntax_tree, identifier.into()),
                    )
                })
                .collect(),
        ),
    }
}

fn nonansi_output_port_block_data(
    syntax_tree: &sv_parser::SyntaxTree,
    declaration: &sv_parser::OutputDeclaration,
) -> Option<Vec<(BlockType, String)>> {
    match declaration {
        sv_parser::OutputDeclaration::Net(declaration) => Some(
            declaration
                .nodes
                .2
                .nodes
                .0
                .contents()
                .into_iter()
                .map(|(identifier, _)| {
                    (
                        BlockType::ModOutput,
                        identifier_text(syntax_tree, identifier.into()),
                    )
                })
                .collect(),
        ),
        sv_parser::OutputDeclaration::Variable(declaration) => Some(
            declaration
                .nodes
                .2
                .nodes
                .0
                .contents()
                .into_iter()
                .map(|(identifier, _, _)| {
                    (
                        BlockType::ModOutput,
                        identifier_text(syntax_tree, identifier.into()),
                    )
                })
                .collect(),
        ),
    }
}

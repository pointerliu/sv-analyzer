use std::collections::{HashMap, HashSet};

use anyhow::Result;
use sv_parser::{unwrap_locate, unwrap_node, RefNode};

use super::{Block, BlockSet, BlockType, Blockizer, CircuitType, DataflowEntry};
use crate::ast::ParsedFile;
use crate::types::{BlockId, SignalNode};

#[derive(Debug, Default)]
pub struct DataflowBlockizer;

impl Blockizer for DataflowBlockizer {
    fn blockize(&self, files: &[ParsedFile]) -> Result<BlockSet> {
        let mut collector = BlockCollector::default();

        for file in files {
            collector.collect_file(file)?;
        }

        BlockSet::new(merge_assign_blocks(collector.blocks)?)
    }
}

#[derive(Debug, Clone)]
struct ModuleTemplate {
    blocks: Vec<Block>,
    port_order: Vec<String>,
}

#[derive(Debug, Clone)]
struct ModuleInstance {
    module_name: String,
    instance_name: String,
    source_file: String,
    line_start: usize,
    line_end: usize,
    code_snippet: String,
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
                    let line_end = line_start;
                    let code_snippet = snippet_from_source(&file.source_text, line_start, line_end);

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
                                source_file: file.path.display().to_string(),
                                line_start,
                                line_end,
                                code_snippet: code_snippet.clone(),
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
        &instance.source_file,
        instance.line_start,
        instance.line_end,
        dataflow,
        &instance.code_snippet,
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
                    if let Some(module_scope) = current_module.as_deref() {
                        self.push_ansi_port_blocks(file, module_scope, module)?;
                    }
                }
                sv_parser::NodeEvent::Enter(RefNode::ModuleDeclarationNonansi(module)) => {
                    current_module = module_name_from_node(&file.syntax_tree, module.into());
                    if let Some(module_scope) = current_module.as_deref() {
                        self.push_nonansi_port_blocks(file, module_scope, module)?;
                    }
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
                            let line_start = start_line(RefNode::ContinuousAssign(assign.into()));
                            let line_end = continuous_assign_end_line(assign).unwrap_or(line_start);
                            let code_snippet =
                                snippet_from_source(&file.source_text, line_start, line_end);
                            self.push_block(
                                file,
                                module_scope,
                                BlockType::Assign,
                                CircuitType::Combinational,
                                line_start,
                                line_end,
                                code_snippet,
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
                            let line_start = start_line(RefNode::AlwaysConstruct(always_construct));
                            let line_end = always_end_line(always_construct).unwrap_or(line_start);
                            let code_snippet =
                                snippet_from_source(&file.source_text, line_start, line_end);
                            self.push_block(
                                file,
                                module_scope,
                                block_type,
                                circuit_type,
                                line_start,
                                line_end,
                                code_snippet,
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
        line_start: usize,
        line_end: usize,
        code_snippet: String,
        dataflow: Vec<DataflowEntry>,
    ) -> Result<()> {
        self.blocks.push(Block::new(
            BlockId(self.next_block_id),
            block_type,
            circuit_type,
            module_scope,
            file.path.display().to_string(),
            line_start,
            line_end,
            dataflow,
            code_snippet,
        )?);
        self.next_block_id += 1;

        Ok(())
    }

    fn push_ansi_port_blocks(
        &mut self,
        file: &ParsedFile,
        module_scope: &str,
        module: &sv_parser::ModuleDeclarationAnsi,
    ) -> Result<()> {
        let Some(port_declarations) = &module.nodes.0.nodes.6 else {
            return Ok(());
        };
        let Some(port_list) = &port_declarations.nodes.0.nodes.1 else {
            return Ok(());
        };

        for (_, port) in port_list.contents() {
            if let Some((block_type, signal_name)) = ansi_port_block_data(&file.syntax_tree, port) {
                self.push_port_block(file, module_scope, block_type, signal_name)?;
            }
        }

        Ok(())
    }

    fn push_nonansi_port_blocks(
        &mut self,
        file: &ParsedFile,
        module_scope: &str,
        module: &sv_parser::ModuleDeclarationNonansi,
    ) -> Result<()> {
        for item in &module.nodes.2 {
            let sv_parser::ModuleItem::PortDeclaration(port_declaration) = item else {
                continue;
            };

            if let Some(ports) = nonansi_port_block_data(&file.syntax_tree, &port_declaration.0) {
                for (block_type, signal_name) in ports {
                    self.push_port_block(file, module_scope, block_type, signal_name)?;
                }
            }
        }

        Ok(())
    }

    fn push_port_block(
        &mut self,
        file: &ParsedFile,
        module_scope: &str,
        block_type: BlockType,
        signal_name: String,
    ) -> Result<()> {
        let line_start = port_block_details(&file.syntax_tree, &signal_name).unwrap_or(1);
        let code_snippet = snippet_from_source(&file.source_text, line_start, line_start);
        let line_end = line_start;
        let dataflow = match block_type {
            BlockType::ModInput => vec![to_entry(vec![signal_name], HashSet::new())],
            BlockType::ModOutput => vec![to_entry(Vec::new(), HashSet::from([signal_name]))],
            _ => return Ok(()),
        };

        self.blocks.push(Block::new(
            BlockId(self.next_block_id),
            block_type,
            CircuitType::Combinational,
            module_scope,
            file.path.display().to_string(),
            line_start,
            line_end,
            dataflow,
            code_snippet,
        )?);
        self.next_block_id += 1;

        Ok(())
    }
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

fn merge_assign_blocks(blocks: Vec<Block>) -> Result<Vec<Block>> {
    let mut assign_blocks = Vec::new();
    let mut other_blocks = Vec::new();

    for block in blocks {
        if matches!(block.block_type(), BlockType::Assign) {
            assign_blocks.push(block);
        } else {
            other_blocks.push(block);
        }
    }

    if assign_blocks.len() < 2 {
        other_blocks.extend(assign_blocks);
        return Ok(other_blocks);
    }

    let mut parents = (0..assign_blocks.len()).collect::<Vec<_>>();

    for left in 0..assign_blocks.len() {
        for right in (left + 1)..assign_blocks.len() {
            if same_assign_merge_group(&assign_blocks[left], &assign_blocks[right])
                && assign_blocks_connected(&assign_blocks[left], &assign_blocks[right])
            {
                union(&mut parents, left, right);
            }
        }
    }

    let mut groups: HashMap<usize, Vec<Block>> = HashMap::new();
    for (index, block) in assign_blocks.into_iter().enumerate() {
        let root = find(&mut parents, index);
        groups.entry(root).or_default().push(block);
    }

    let mut merged_assigns = groups
        .into_values()
        .map(merge_assign_group)
        .collect::<Result<Vec<_>>>()?;

    other_blocks.append(&mut merged_assigns);
    Ok(other_blocks)
}

fn same_assign_merge_group(left: &Block, right: &Block) -> bool {
    left.module_scope() == right.module_scope()
        && left.source_file() == right.source_file()
        && left.circuit_type() == right.circuit_type()
}

fn assign_blocks_connected(left: &Block, right: &Block) -> bool {
    !left.output_signals().is_disjoint(right.input_signals())
        || !right.output_signals().is_disjoint(left.input_signals())
}

fn merge_assign_group(mut blocks: Vec<Block>) -> Result<Block> {
    if blocks.len() == 1 {
        return Ok(blocks.pop().unwrap());
    }

    blocks.sort_by_key(|block| block.id().0);

    let first = &blocks[0];
    let line_start = blocks.iter().map(Block::line_start).min().unwrap_or(1);
    let line_end = blocks
        .iter()
        .map(Block::line_end)
        .max()
        .unwrap_or(line_start);
    let raw_entries = blocks
        .iter()
        .flat_map(|block| block.dataflow().iter().cloned())
        .collect::<Vec<_>>();
    let internal_outputs = raw_entries
        .iter()
        .flat_map(|entry| entry.output.iter().cloned())
        .collect::<HashSet<_>>();
    let entry_inputs = signal_inputs_by_output(&raw_entries);
    let dataflow = raw_entries
        .into_iter()
        .map(|entry| DataflowEntry {
            output: entry.output.clone(),
            inputs: resolve_external_inputs(&entry.inputs, &internal_outputs, &entry_inputs),
        })
        .collect::<Vec<_>>();
    let code_snippet = blocks
        .iter()
        .map(|block| block.code_snippet().trim())
        .filter(|snippet| !snippet.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    Block::new(
        first.id(),
        BlockType::Assign,
        first.circuit_type(),
        first.module_scope(),
        first.source_file(),
        line_start,
        line_end,
        dataflow,
        code_snippet,
    )
}

fn resolve_external_inputs(
    inputs: &HashSet<crate::types::SignalNode>,
    internal_outputs: &HashSet<crate::types::SignalNode>,
    entry_inputs: &HashMap<crate::types::SignalNode, HashSet<crate::types::SignalNode>>,
) -> HashSet<crate::types::SignalNode> {
    let mut resolved = HashSet::new();

    for input in inputs {
        expand_input(
            input,
            internal_outputs,
            entry_inputs,
            &mut HashSet::new(),
            &mut resolved,
        );
    }

    resolved
}

fn expand_input(
    input: &crate::types::SignalNode,
    internal_outputs: &HashSet<crate::types::SignalNode>,
    entry_inputs: &HashMap<crate::types::SignalNode, HashSet<crate::types::SignalNode>>,
    visiting: &mut HashSet<crate::types::SignalNode>,
    resolved: &mut HashSet<crate::types::SignalNode>,
) {
    if !internal_outputs.contains(input) {
        resolved.insert(input.clone());
        return;
    }

    if !visiting.insert(input.clone()) {
        return;
    }

    if let Some(upstream_inputs) = entry_inputs.get(input) {
        for upstream_input in upstream_inputs {
            expand_input(
                upstream_input,
                internal_outputs,
                entry_inputs,
                visiting,
                resolved,
            );
        }
    }

    visiting.remove(input);
}

fn find(parents: &mut [usize], index: usize) -> usize {
    if parents[index] != index {
        let root = find(parents, parents[index]);
        parents[index] = root;
    }
    parents[index]
}

fn union(parents: &mut [usize], left: usize, right: usize) {
    let left_root = find(parents, left);
    let right_root = find(parents, right);
    if left_root != right_root {
        parents[right_root] = left_root;
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

fn to_entry(outputs: Vec<String>, inputs: HashSet<String>) -> DataflowEntry {
    DataflowEntry {
        output: outputs
            .into_iter()
            .map(crate::types::SignalNode::named)
            .collect(),
        inputs: inputs
            .into_iter()
            .map(crate::types::SignalNode::named)
            .collect(),
    }
}

fn signal_inputs_by_output(
    entries: &[DataflowEntry],
) -> HashMap<crate::types::SignalNode, HashSet<crate::types::SignalNode>> {
    let mut map = HashMap::new();
    for entry in entries {
        for output in &entry.output {
            map.insert(output.clone(), entry.inputs.clone());
        }
    }
    map
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

use std::collections::BTreeSet;

use crate::config::{Config, Node};
use crate::graph::ImpactPlan;
use crate::runner::display_command;

#[derive(Debug)]
struct TreeNode {
    label: String,
    children: Vec<TreeNode>,
}

impl TreeNode {
    fn leaf(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            children: Vec::new(),
        }
    }

    fn branch(label: impl Into<String>, children: Vec<TreeNode>) -> Self {
        Self {
            label: label.into(),
            children,
        }
    }
}

pub fn render_node_list(config: &Config) -> String {
    let mut output = String::new();

    for (name, node) in config.nodes() {
        output.push_str(name);
        output.push_str(": ");
        output.push_str(&display_command(&node.command));
        output.push('\n');
    }

    output
}

pub fn render_plan(config: &Config, plan: &ImpactPlan) -> String {
    let mut output = String::new();

    output.push_str("Changed files:\n");
    push_items(&mut output, &plan.changed_files);

    output.push_str("\nDirect impact:\n");
    let direct: Vec<String> = plan.direct_matches.keys().cloned().collect();
    push_items(&mut output, &direct);

    output.push_str("\nExpanded impact:\n");
    let impacted: Vec<String> = plan.impacted.iter().cloned().collect();
    push_items(&mut output, &impacted);

    output.push_str("\nExecution order:\n");
    if plan.execution_order.is_empty() {
        output.push_str("  none\n");
    } else {
        for name in &plan.execution_order {
            let node = config
                .node(name)
                .expect("plans only contain configured nodes");
            output.push_str("  ");
            output.push_str(name);
            output.push_str(": ");
            output.push_str(&display_command(&node.command));
            output.push('\n');
        }
    }

    output
}

pub fn render_config_tree(config: &Config) -> String {
    let children = config
        .nodes()
        .iter()
        .map(|(name, node)| render_config_node(name, node))
        .collect();

    render_tree(&TreeNode::branch("git-impact.yaml", children))
}

pub fn render_impact_tree(config: &Config, plan: &ImpactPlan) -> String {
    let mut children = Vec::new();

    children.push(TreeNode::branch(
        "changed files",
        plan.changed_files
            .iter()
            .cloned()
            .map(TreeNode::leaf)
            .collect(),
    ));

    children.push(TreeNode::branch(
        "impact expansion",
        render_expansion_roots(config, plan),
    ));

    children.push(TreeNode::branch(
        "execution order",
        plan.execution_order
            .iter()
            .map(|name| {
                let node = config
                    .node(name)
                    .expect("plans only contain configured nodes");
                TreeNode::leaf(format!("{name}: {}", display_command(&node.command)))
            })
            .collect(),
    ));

    render_tree(&TreeNode::branch("git-impact", children))
}

fn render_config_node(name: &str, node: &Node) -> TreeNode {
    let mut children = Vec::new();

    if !node.paths.is_empty() {
        children.push(TreeNode::branch(
            "paths",
            node.paths.iter().cloned().map(TreeNode::leaf).collect(),
        ));
    }

    if !node.depends_on.is_empty() {
        children.push(TreeNode::branch(
            "depends_on",
            node.depends_on
                .iter()
                .cloned()
                .map(TreeNode::leaf)
                .collect(),
        ));
    }

    children.push(TreeNode::leaf(format!(
        "command: {}",
        display_command(&node.command)
    )));

    TreeNode::branch(name, children)
}

fn render_expansion_roots(config: &Config, plan: &ImpactPlan) -> Vec<TreeNode> {
    let mut rendered = BTreeSet::new();
    plan.direct_matches
        .keys()
        .map(|name| render_expansion_node(config, plan, name, &mut rendered))
        .collect()
}

fn render_expansion_node(
    config: &Config,
    plan: &ImpactPlan,
    name: &str,
    rendered: &mut BTreeSet<String>,
) -> TreeNode {
    if !rendered.insert(name.to_owned()) {
        return TreeNode::leaf(format!("{name} [already shown]"));
    }

    let node = config
        .node(name)
        .expect("plans only contain configured nodes");
    let mut children = Vec::new();

    if let Some(matches) = plan.direct_matches.get(name) {
        children.push(TreeNode::branch(
            "matched files",
            matches.iter().cloned().map(TreeNode::leaf).collect(),
        ));
    }

    children.push(TreeNode::leaf(format!(
        "command: {}",
        display_command(&node.command)
    )));

    let downstream: Vec<TreeNode> = plan
        .triggered_by
        .iter()
        .filter(|(_, sources)| sources.contains(name))
        .map(|(dependent, _)| render_expansion_node(config, plan, dependent, rendered))
        .collect();

    if !downstream.is_empty() {
        children.push(TreeNode::branch("downstream", downstream));
    }

    let tag = if plan.direct_matches.contains_key(name) {
        "direct"
    } else {
        "impacted"
    };

    TreeNode::branch(format!("{name} [{tag}]"), children)
}

fn render_tree(root: &TreeNode) -> String {
    let mut output = String::new();
    output.push_str(&root.label);
    output.push('\n');

    for (index, child) in root.children.iter().enumerate() {
        let is_last = index + 1 == root.children.len();
        render_tree_child(child, "", is_last, &mut output);
    }

    output
}

fn render_tree_child(node: &TreeNode, prefix: &str, is_last: bool, output: &mut String) {
    output.push_str(prefix);
    output.push_str(if is_last { "`-- " } else { "|-- " });
    output.push_str(&node.label);
    output.push('\n');

    let child_prefix = if is_last {
        format!("{prefix}    ")
    } else {
        format!("{prefix}|   ")
    };

    for (index, child) in node.children.iter().enumerate() {
        let child_is_last = index + 1 == node.children.len();
        render_tree_child(child, &child_prefix, child_is_last, output);
    }
}

fn push_items(output: &mut String, items: &[String]) {
    if items.is_empty() {
        output.push_str("  none\n");
        return;
    }

    for item in items {
        output.push_str("  ");
        output.push_str(item);
        output.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use crate::graph::Planner;

    use super::*;

    #[test]
    fn renders_tree_style_impact_output() {
        let config = Config::from_yaml(
            r#"
nodes:
  catalog:
    paths:
      - modules/datalake/catalog/**
    command:
      - terragrunt
      - apply
  dagster:
    paths:
      - dagster/**
    depends_on:
      - catalog
    command:
      - make
      - deploy
"#,
        )
        .unwrap();
        let planner = Planner::new(&config).unwrap();
        let plan = planner.plan(vec!["modules/datalake/catalog/schema.sql".to_owned()]);

        let output = render_impact_tree(&config, &plan);

        assert!(output.contains("|-- changed files"));
        assert!(output.contains("catalog [direct]"));
        assert!(output.contains("dagster [impacted]"));
        assert!(output.contains("command: terragrunt apply"));
    }
}

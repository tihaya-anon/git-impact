use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use globset::GlobBuilder;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct Config {
    nodes: BTreeMap<String, Node>,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub paths: Vec<String>,
    pub depends_on: Vec<String>,
    pub command: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawConfig {
    #[serde(default)]
    nodes: BTreeMap<String, RawNode>,

    #[serde(flatten)]
    direct_nodes: BTreeMap<String, RawNode>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNode {
    #[serde(default)]
    paths: Vec<String>,

    #[serde(default)]
    depends_on: Vec<String>,

    command: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VisitState {
    Visiting,
    Visited,
}

impl Config {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let content = fs::read_to_string(path)
            .with_context(|| format!("failed to read config {}", path.display()))?;
        Self::from_yaml(&content).with_context(|| format!("invalid config {}", path.display()))
    }

    pub fn from_yaml(content: &str) -> Result<Self> {
        let raw: RawConfig = serde_yaml::from_str(content)?;
        let raw_nodes = select_nodes(raw)?;

        if raw_nodes.is_empty() {
            bail!("config must define at least one node");
        }

        let mut nodes = BTreeMap::new();
        for (name, raw_node) in raw_nodes {
            validate_node_name(&name)?;
            validate_node(&name, &raw_node)?;
            nodes.insert(
                name,
                Node {
                    paths: raw_node.paths,
                    depends_on: raw_node.depends_on,
                    command: raw_node.command,
                },
            );
        }

        validate_dependencies(&nodes)?;
        validate_acyclic(&nodes)?;

        Ok(Self { nodes })
    }

    pub fn nodes(&self) -> &BTreeMap<String, Node> {
        &self.nodes
    }

    pub fn node(&self, name: &str) -> Option<&Node> {
        self.nodes.get(name)
    }
}

fn select_nodes(raw: RawConfig) -> Result<BTreeMap<String, RawNode>> {
    match (raw.nodes.is_empty(), raw.direct_nodes.is_empty()) {
        (false, false) => bail!("use either top-level 'nodes:' or direct node keys, not both"),
        (false, true) => Ok(raw.nodes),
        (true, false) => Ok(raw.direct_nodes),
        (true, true) => Ok(BTreeMap::new()),
    }
}

fn validate_node_name(name: &str) -> Result<()> {
    if name.trim().is_empty() {
        bail!("node name cannot be empty");
    }

    Ok(())
}

fn validate_node(name: &str, node: &RawNode) -> Result<()> {
    if node.command.is_empty() {
        bail!("node '{name}' command cannot be empty");
    }

    for arg in &node.command {
        if arg.trim().is_empty() {
            bail!("node '{name}' command contains an empty argument");
        }
    }

    for pattern in &node.paths {
        if pattern.trim().is_empty() {
            bail!("node '{name}' contains an empty path pattern");
        }

        GlobBuilder::new(pattern)
            .literal_separator(true)
            .build()
            .with_context(|| format!("node '{name}' has invalid glob '{pattern}'"))?;
    }

    Ok(())
}

fn validate_dependencies(nodes: &BTreeMap<String, Node>) -> Result<()> {
    for (name, node) in nodes {
        let mut seen = HashSet::new();
        for dependency in &node.depends_on {
            if !nodes.contains_key(dependency) {
                bail!("node '{name}' depends on unknown node '{dependency}'");
            }

            if !seen.insert(dependency) {
                bail!("node '{name}' depends on '{dependency}' more than once");
            }
        }
    }

    Ok(())
}

fn validate_acyclic(nodes: &BTreeMap<String, Node>) -> Result<()> {
    let mut states = HashMap::new();
    let mut stack = Vec::new();

    for name in nodes.keys() {
        visit_for_cycle(name, nodes, &mut states, &mut stack)?;
    }

    Ok(())
}

fn visit_for_cycle(
    name: &str,
    nodes: &BTreeMap<String, Node>,
    states: &mut HashMap<String, VisitState>,
    stack: &mut Vec<String>,
) -> Result<()> {
    match states.get(name) {
        Some(VisitState::Visited) => return Ok(()),
        Some(VisitState::Visiting) => {
            let cycle_start = stack.iter().position(|item| item == name).unwrap_or(0);
            let mut cycle = stack[cycle_start..].to_vec();
            cycle.push(name.to_owned());
            bail!("dependency cycle detected: {}", cycle.join(" -> "));
        }
        None => {}
    }

    states.insert(name.to_owned(), VisitState::Visiting);
    stack.push(name.to_owned());

    let node = nodes
        .get(name)
        .expect("dependency validation ensures referenced nodes exist");
    for dependency in &node.depends_on {
        visit_for_cycle(dependency, nodes, states, stack)?;
    }

    stack.pop();
    states.insert(name.to_owned(), VisitState::Visited);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_wrapped_nodes_config() {
        let config = Config::from_yaml(
            r#"
nodes:
  catalog:
    paths:
      - modules/datalake/catalog/**
    command:
      - terragrunt
      - apply
"#,
        )
        .unwrap();

        assert!(config.node("catalog").is_some());
    }

    #[test]
    fn accepts_direct_nodes_config() {
        let config = Config::from_yaml(
            r#"
catalog:
  paths:
    - modules/datalake/catalog/**
  command:
    - terragrunt
    - apply
"#,
        )
        .unwrap();

        assert!(config.node("catalog").is_some());
    }

    #[test]
    fn rejects_unknown_dependencies() {
        let error = Config::from_yaml(
            r#"
nodes:
  dagster:
    paths:
      - dagster/**
    depends_on:
      - catalog
    command:
      - make
"#,
        )
        .unwrap_err();

        assert!(error.to_string().contains("unknown node 'catalog'"));
    }

    #[test]
    fn rejects_dependency_cycles() {
        let error = Config::from_yaml(
            r#"
nodes:
  a:
    depends_on:
      - b
    command:
      - echo
      - a
  b:
    depends_on:
      - a
    command:
      - echo
      - b
"#,
        )
        .unwrap_err();

        assert!(error.to_string().contains("dependency cycle detected"));
    }
}

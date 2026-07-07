use std::collections::{BTreeMap, BTreeSet, HashMap};

use anyhow::{Context, Result};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};

use crate::config::{Config, Node};

#[derive(Debug)]
pub struct Planner {
    nodes: BTreeMap<String, CompiledNode>,
    dependents: BTreeMap<String, Vec<String>>,
}

#[derive(Debug)]
struct CompiledNode {
    spec: Node,
    globs: GlobSet,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ImpactPlan {
    pub changed_files: Vec<String>,
    pub direct_matches: BTreeMap<String, Vec<String>>,
    pub triggered_by: BTreeMap<String, BTreeSet<String>>,
    pub impacted: BTreeSet<String>,
    pub execution_order: Vec<String>,
}

impl Planner {
    pub fn new(config: &Config) -> Result<Self> {
        let mut nodes = BTreeMap::new();
        let mut dependents: BTreeMap<String, Vec<String>> = BTreeMap::new();

        for (name, node) in config.nodes() {
            nodes.insert(
                name.clone(),
                CompiledNode {
                    spec: node.clone(),
                    globs: compile_globs(name, &node.paths)?,
                },
            );

            dependents.entry(name.clone()).or_default();
            for dependency in &node.depends_on {
                dependents
                    .entry(dependency.clone())
                    .or_default()
                    .push(name.clone());
            }
        }

        for values in dependents.values_mut() {
            values.sort();
            values.dedup();
        }

        Ok(Self { nodes, dependents })
    }

    pub fn plan(&self, changed_files: Vec<String>) -> ImpactPlan {
        let changed_files = sorted_unique(changed_files);
        let direct_matches = self.direct_matches(&changed_files);

        let mut impacted: BTreeSet<String> = direct_matches.keys().cloned().collect();
        let mut triggered_by: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut queue: Vec<String> = impacted.iter().cloned().collect();
        let mut cursor = 0;

        while cursor < queue.len() {
            let current = queue[cursor].clone();
            cursor += 1;

            for dependent in self.dependents.get(&current).into_iter().flatten() {
                triggered_by
                    .entry(dependent.clone())
                    .or_default()
                    .insert(current.clone());

                if impacted.insert(dependent.clone()) {
                    queue.push(dependent.clone());
                }
            }
        }

        let execution_order = self.execution_order(&impacted);

        ImpactPlan {
            changed_files,
            direct_matches,
            triggered_by,
            impacted,
            execution_order,
        }
    }

    fn direct_matches(&self, changed_files: &[String]) -> BTreeMap<String, Vec<String>> {
        let mut matches = BTreeMap::new();

        for (name, node) in &self.nodes {
            let node_matches: Vec<String> = changed_files
                .iter()
                .filter(|path| node.globs.is_match(path.as_str()))
                .cloned()
                .collect();

            if !node_matches.is_empty() {
                matches.insert(name.clone(), node_matches);
            }
        }

        matches
    }

    fn execution_order(&self, impacted: &BTreeSet<String>) -> Vec<String> {
        let mut order = Vec::new();
        let mut visited = BTreeSet::new();

        for name in impacted {
            self.visit_for_order(name, impacted, &mut visited, &mut order);
        }

        order
    }

    fn visit_for_order(
        &self,
        name: &str,
        impacted: &BTreeSet<String>,
        visited: &mut BTreeSet<String>,
        order: &mut Vec<String>,
    ) {
        if !visited.insert(name.to_owned()) {
            return;
        }

        let node = self
            .nodes
            .get(name)
            .expect("impacted nodes are selected from configured nodes");
        for dependency in &node.spec.depends_on {
            if impacted.contains(dependency) {
                self.visit_for_order(dependency, impacted, visited, order);
            }
        }

        order.push(name.to_owned());
    }
}

fn compile_globs(name: &str, patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();

    for pattern in patterns {
        let glob = GlobBuilder::new(pattern)
            .literal_separator(true)
            .build()
            .with_context(|| format!("node '{name}' has invalid glob '{pattern}'"))?;
        builder.add(glob);
    }

    builder
        .build()
        .with_context(|| format!("failed to compile glob set for node '{name}'"))
}

fn sorted_unique(values: Vec<String>) -> Vec<String> {
    let mut seen = HashMap::new();
    for value in values {
        seen.insert(value, ());
    }

    let mut values: Vec<String> = seen.into_keys().collect();
    values.sort();
    values
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config() -> Config {
        Config::from_yaml(
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
  reports:
    paths:
      - reports/**
    depends_on:
      - dagster
    command:
      - make
      - reports
"#,
        )
        .unwrap()
    }

    #[test]
    fn expands_downstream_dependents() {
        let planner = Planner::new(&sample_config()).unwrap();
        let plan = planner.plan(vec!["modules/datalake/catalog/schema.sql".to_owned()]);

        assert_eq!(
            plan.execution_order,
            vec![
                "catalog".to_owned(),
                "dagster".to_owned(),
                "reports".to_owned()
            ]
        );
        assert!(plan.direct_matches.contains_key("catalog"));
        assert!(plan.impacted.contains("dagster"));
        assert!(plan.impacted.contains("reports"));
    }

    #[test]
    fn does_not_pull_in_upstream_dependencies_for_downstream_changes() {
        let planner = Planner::new(&sample_config()).unwrap();
        let plan = planner.plan(vec!["dagster/job.py".to_owned()]);

        assert_eq!(
            plan.execution_order,
            vec!["dagster".to_owned(), "reports".to_owned()]
        );
        assert!(!plan.impacted.contains("catalog"));
    }

    #[test]
    fn keeps_dependency_order_when_multiple_nodes_match_directly() {
        let planner = Planner::new(&sample_config()).unwrap();
        let plan = planner.plan(vec![
            "dagster/job.py".to_owned(),
            "modules/datalake/catalog/schema.sql".to_owned(),
        ]);

        assert_eq!(
            plan.execution_order,
            vec![
                "catalog".to_owned(),
                "dagster".to_owned(),
                "reports".to_owned()
            ]
        );
    }
}

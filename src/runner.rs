use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::config::Config;
use crate::graph::ImpactPlan;

pub fn run_plan(config: &Config, plan: &ImpactPlan, dry_run: bool) -> Result<()> {
    if plan.execution_order.is_empty() {
        println!("No impacted commands to run.");
        return Ok(());
    }

    for name in &plan.execution_order {
        let node = config
            .node(name)
            .expect("plans only contain configured nodes");
        let command = display_command(&node.command);

        if dry_run {
            println!("dry-run: {name}: {command}");
            continue;
        }

        println!("running: {name}: {command}");
        let status = Command::new(&node.command[0])
            .args(&node.command[1..])
            .status()
            .with_context(|| format!("failed to start command for node '{name}'"))?;

        if !status.success() {
            let code = status
                .code()
                .map(|code| code.to_string())
                .unwrap_or_else(|| "signal".to_owned());
            bail!("command for node '{name}' failed with status {code}");
        }
    }

    Ok(())
}

pub fn display_command(command: &[String]) -> String {
    command
        .iter()
        .map(|arg| {
            if arg.chars().any(char::is_whitespace) {
                format!("{arg:?}")
            } else {
                arg.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

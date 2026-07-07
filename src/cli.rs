use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::config::Config;
use crate::git::{self, DiffRange};
use crate::graph::{ImpactPlan, Planner};
use crate::init;
use crate::render;
use crate::runner;

#[derive(Debug, Parser)]
#[command(name = "git-impact")]
#[command(about = "Run commands for Git changes expanded through a dependency graph")]
#[command(version)]
struct Cli {
    #[arg(short, long, default_value = "git-impact.yaml")]
    config: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Create a ready-to-run config file at the nearest Git repo root.
    Init(InitArgs),

    /// Validate the graph config and exit.
    Validate,

    /// List configured nodes.
    List,

    /// Print impacted nodes and execution order.
    Plan(DiffArgs),

    /// Print a tree-style graph or impact view.
    Tree(TreeArgs),

    /// Execute commands for impacted nodes.
    Run(RunArgs),
}

#[derive(Debug, Args)]
struct InitArgs {
    /// Overwrite an existing config file.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Args, Clone)]
struct DiffArgs {
    #[arg(long, default_value = "origin/main")]
    base: String,

    #[arg(long, default_value = "HEAD")]
    head: String,

    #[arg(long, value_enum, default_value_t = RangeArg::ThreeDot)]
    range: RangeArg,
}

#[derive(Debug, Args)]
struct TreeArgs {
    #[command(flatten)]
    diff: Option<DiffArgs>,

    /// Show the configured dependency graph instead of a Git impact tree.
    #[arg(long)]
    graph: bool,
}

#[derive(Debug, Args)]
struct RunArgs {
    #[command(flatten)]
    diff: DiffArgs,

    /// Print commands without executing them.
    #[arg(long)]
    dry_run: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum RangeArg {
    /// Diff base..head.
    TwoDot,

    /// Diff base...head, using Git's merge-base behavior.
    ThreeDot,
}

impl From<RangeArg> for DiffRange {
    fn from(value: RangeArg) -> Self {
        match value {
            RangeArg::TwoDot => DiffRange::TwoDot,
            RangeArg::ThreeDot => DiffRange::ThreeDot,
        }
    }
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Init(args) => {
            let result = init::init_config(&cli.config, args.force)?;
            println!("created {}", result.path.display());
            if !result.has_patterns {
                println!(
                    "no repo files found; the config contains a placeholder node with no paths. Add file patterns after adding files, or run `git-impact init --force` later."
                );
            }
        }
        Command::Validate => {
            let config = Config::from_path(&cli.config)?;
            Planner::new(&config)?;
            println!("valid config: {}", cli.config.display());
        }
        Command::List => {
            let config = Config::from_path(&cli.config)?;
            print!("{}", render::render_node_list(&config));
        }
        Command::Plan(args) => {
            let config = Config::from_path(&cli.config)?;
            let planner = Planner::new(&config)?;
            let plan = build_plan(&planner, args)?;
            print!("{}", render::render_plan(&config, &plan));
        }
        Command::Tree(args) => {
            let config = Config::from_path(&cli.config)?;
            let planner = Planner::new(&config)?;
            if args.graph {
                print!("{}", render::render_config_tree(&config));
            } else {
                let diff = args.diff.unwrap_or_else(default_diff_args);
                let plan = build_plan(&planner, diff)?;
                print!("{}", render::render_impact_tree(&config, &plan));
            }
        }
        Command::Run(args) => {
            let config = Config::from_path(&cli.config)?;
            let planner = Planner::new(&config)?;
            let plan = build_plan(&planner, args.diff)?;
            print!("{}", render::render_plan(&config, &plan));
            runner::run_plan(&config, &plan, args.dry_run)?;
        }
    }

    Ok(())
}

fn build_plan(planner: &Planner, args: DiffArgs) -> Result<ImpactPlan> {
    let changed_files = git::changed_files(&args.base, &args.head, args.range.into())?;
    Ok(planner.plan(changed_files))
}

fn default_diff_args() -> DiffArgs {
    DiffArgs {
        base: "origin/main".to_owned(),
        head: "HEAD".to_owned(),
        range: RangeArg::ThreeDot,
    }
}

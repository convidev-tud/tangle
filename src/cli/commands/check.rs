use crate::cli::*;
use crate::git::conflict::{ConflictChecker, ConflictStatistics};
use crate::model::{NodePathType, QualifiedPath};
use clap::{Arg, ArgAction, Command};
use colored::Colorize;
use std::error::Error;

const SOURCE: &str = "source";
const TARGETS: &str = "targets";
const ALL: &str = "all";

#[derive(Clone, Debug)]
pub struct CheckCommand;

impl CommandDefinition for CheckCommand {
    fn build_command(&self) -> Command {
        Command::new("check")
            .about("Check features for merge conflicts")
            .disable_help_subcommand(true)
            .arg(Arg::new(SOURCE).help("Feature to check against targets"))
            .arg(Arg::new(TARGETS).help(
                "Targets to check against; If none are provided, will check against all features",
            ))
            .arg(
                Arg::new(ALL)
                    .long("all")
                    .action(ArgAction::SetTrue)
                    .help("Check all features against each other"),
            )
            .arg(verbose())
    }
}

impl CommandInterface for CheckCommand {
    fn run_command(&self, context: &mut CommandContext) -> Result<(), Box<dyn Error>> {
        let feature_root = match context.git.get_current_area()?.to_feature_root() {
            Some(path) => path,
            None => return Err("Nothing to check: no features exist".into()),
        };
        let current_path = context.git.get_current_node_path()?;
        let all = context
            .arg_helper
            .get_argument_value::<bool>(ALL)
            .unwrap_or(false);
        let maybe_feature = context.arg_helper.get_argument_value::<String>(SOURCE);
        let maybe_targets = context.arg_helper.get_argument_values::<String>(TARGETS);
        let checker = ConflictChecker::new(&context.git);

        let statistics: ConflictStatistics = match (all, maybe_feature, maybe_targets) {
            // all AND source are not set => error
            (false, None, _) => return Err("Feature must be provided if --all is not set".into()),
            // all is set => check all
            (true, _, _) => {
                let all_features: Vec<QualifiedPath> = feature_root
                    .iter_children_req()
                    .map(|child| child.get_qualified_path())
                    .collect();
                checker
                    .check_all(&all_features)?
                    .map(|statistic| {
                        context.debug(statistic.to_string());
                        statistic
                    })
                    .collect()
            }
            // all is not set, source is set, target not => check source against all
            (false, Some(source), None) => {
                let qualified_source =
                    current_path.get_qualified_path() + QualifiedPath::from(source);
                match context
                    .git
                    .get_model()
                    .get_node_path(&qualified_source)
                    .unwrap()
                    .concretize()
                {
                    NodePathType::Feature(_) => {}
                    _ => {
                        return Err(format!("{} is not a feature", qualified_source).into());
                    }
                }
                let all_other_features: Vec<QualifiedPath> = feature_root
                    .iter_children_req()
                    .filter_map(|child| {
                        let path = child.get_qualified_path();
                        if path != qualified_source {
                            Some(path)
                        } else {
                            None
                        }
                    })
                    .collect();
                checker
                    .check_1_to_n(&qualified_source, &all_other_features)?
                    .map(|statistic| {
                        context.debug(statistic.to_string());
                        statistic
                    })
                    .collect()
            }
            (false, Some(source), Some(targets)) => {
                let qualified_source =
                    feature_root.get_qualified_path() + QualifiedPath::from(source);
                let qualified_targets: Vec<QualifiedPath> =
                    targets.into_iter().map(QualifiedPath::from).collect();
                checker
                    .check_1_to_n(&qualified_source, &qualified_targets)?
                    .map(|statistic| {
                        context.debug(statistic.to_string());
                        statistic
                    })
                    .collect()
            }
        };
        if statistics.n_conflict() == 0 {
            context.info("No conflicts".green().to_string());
        } else {
            for conflict in statistics.iter_conflicts() {
                context.info(conflict);
            }
        }
        Ok(())
    }
}

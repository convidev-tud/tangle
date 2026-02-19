use crate::cli::*;
use crate::model::{NodePathType, QualifiedPath};
use clap::{Arg, Command};
use std::error::Error;

fn extract_feature_names(message: &str) -> Vec<QualifiedPath> {
    let to_filter = vec!["# DO NOT EDIT OR REMOVE THIS COMMIT", "DERIVATION FINISHED"];
    let trimmed = message.trim();
    trimmed
        .split("\n")
        .filter_map(|e| {
            if !to_filter.contains(&e) {
                Some(QualifiedPath::from(e))
            } else {
                None
            }
        })
        .collect()
}

#[derive(Clone, Debug)]
pub struct UntieCommand;

impl CommandDefinition for UntieCommand {
    fn build_command(&self) -> Command {
        Command::new("untie")
            .about("Untie commit from product and merge back into feature")
            .disable_help_subcommand(true)
            .arg(
                Arg::new("commit")
                    .short('c')
                    .long("commit")
                    .help("Specific commit to untie"),
            )
            .arg(
                Arg::new("feature")
                    .short('f')
                    .long("feature")
                    .help("Feature to untie to"),
            )
    }
}

impl CommandInterface for UntieCommand {
    fn run_command(&self, context: &mut CommandContext) -> Result<(), Box<dyn Error>> {
        let current = match context.git.get_current_node_path()?.concretize() {
            NodePathType::Product(path) => path,
            _ => {
                return Err("Not on product branch".into());
            }
        };
        let maybe_commit = context.arg_helper.get_argument_value::<String>("commit");
        let maybe_feature = context.arg_helper.get_argument_value::<String>("feature");
        let mut commit_history = context
            .git
            .get_commit_history(&current.get_qualified_path())?;
        if commit_history.is_empty() {
            context.info("No commits on product");
            return Ok(());
        }
        let hash: String = match maybe_commit {
            Some(commit) => commit,
            None => commit_history.get(0).unwrap().hash().clone(),
        };
        let mut has_valid = false;
        let mut derivation_found = false;
        let mut features: Vec<QualifiedPath> = Vec::new();
        commit_history.reverse();
        for commit in commit_history.iter() {
            if commit.message().contains("DERIVATION FINISHED") {
                if commit.hash() == &hash {
                    return Err("Derivation commit cannot be untied".into());
                }
                derivation_found = true;
                features.extend(extract_feature_names(&commit.message()));
            } else {
                if derivation_found && commit.hash() == &hash {
                    has_valid = true;
                    break;
                }
            }
        }
        if !has_valid {
            return Err("Commit not found after initial derivation".into());
        }
        let files_of_commit = context.git.get_files_changed_by_commit(&hash)?;
        let filtered = features
            .into_iter()
            .filter(|feature| {
                let managed_files = context.git.get_files_managed_by_branch(feature).unwrap();
                let mut all_true = true;
                for file_in_commit in files_of_commit.iter() {
                    if !managed_files.contains(file_in_commit) {
                        all_true = false;
                    }
                }
                all_true
            })
            .collect::<Vec<QualifiedPath>>();
        let feature: QualifiedPath = match maybe_feature {
            Some(feature) => QualifiedPath::from(feature),
            None => {
                match filtered.len() {
                    0 => { return Err("There are no features matching all changed files. Please choose one manually with the --feature parameter.".into()) }
                    1 => { QualifiedPath::from(filtered[0].clone()) },
                    _ => { return Err("There are multiple potential untie targets. Please choose one manually with the --feature parameter.".into() ) }
                }
            },
        };
        let current_path = context.git.get_current_qualified_path()?;
        context.git.checkout(&feature)?;
        let output = context.git.cherry_pick(&hash)?;
        if !output.status.success() {
            context.git.abort_merge()?;
            context.info(format!("Unable to untie commit {}", &hash));
        } else {
            context.info(format!("Untied commit {} to {}", &hash, &feature));
        }
        context.git.checkout(&current_path)?;
        Ok(())
    }
}

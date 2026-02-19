use crate::cli::*;
use crate::git::conflict::ConflictChecker;
use crate::model::QualifiedPath;
use clap::Command;
use std::error::Error;

fn no_features() -> String {
    "Nothing to check: no features in tree".to_string()
}

#[derive(Clone, Debug)]
pub struct CheckCommand;

impl CommandDefinition for CheckCommand {
    fn build_command(&self) -> Command {
        Command::new("check")
            .about("Check all features for merge conflicts")
            .disable_help_subcommand(true)
            .arg(verbose())
    }
}

impl CommandInterface for CheckCommand {
    fn run_command(&self, context: &mut CommandContext) -> Result<(), Box<dyn Error>> {
        let maybe_feature_root = context.git.get_current_area()?.to_feature_root();
        if maybe_feature_root.is_some() {
            let feature_root = maybe_feature_root.unwrap();
            let all_features: Vec<QualifiedPath> = feature_root
                .iter_children_req()
                .map(|child| child.get_qualified_path())
                .collect();
            for statistic in ConflictChecker::new(&context.git).check(&all_features)? {
                context.debug(statistic);
            }
        } else {
            context.info(no_features());
        }
        Ok(())
    }
}

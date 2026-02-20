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
            .arg(Arg::new(TARGETS).action(ArgAction::Append).help(
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
                checker.check_all(&all_features)?.collect()
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
                    .collect()
            }
            (false, Some(source), Some(targets)) => {
                let qualified_source =
                    current_path.get_qualified_path() + QualifiedPath::from(source);
                let qualified_targets: Vec<QualifiedPath> = targets
                    .into_iter()
                    .map(|target| feature_root.get_qualified_path() + QualifiedPath::from(target))
                    .collect();
                checker
                    .check_1_to_n(&qualified_source, &qualified_targets)?
                    .collect()
            }
        };
        for ok in statistics.iter_ok() {
            context.debug(ok)
        }
        for conflict in statistics.iter_conflicts() {
            context.warn(conflict)
        }
        for error in statistics.iter_errors() {
            context.error(error)
        }
        if statistics.n_conflict() == 0 {
            context.info("No conflicts".green().to_string());
        }
        Ok(())
    }

    // fn shell_complete(
    //     &self,
    //     completion_helper: CompletionHelper,
    //     context: &mut CommandContext,
    // ) -> Result<Vec<String>, Box<dyn Error>> {
    //     let currently_editing = completion_helper.currently_editing();
    //     let completion: Vec<String> = if currently_editing.is_some() {
    //         match currently_editing.unwrap().get_id().as_str() {
    //             SOURCE => {}
    //             TARGETS => {}
    //             _ => { vec![] }
    //         }
    //     } else { vec![] };
    //     Ok(completion)
    // }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::interface::test_utils::*;
    use crate::git::interface::{GitInterface, GitPath};
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn check_error_if_not_all_and_no_source() {
        let path = TempDir::new().unwrap();
        let path_buf = PathBuf::from(path.path());
        prepare_empty_git_repo(path_buf.clone()).unwrap();
        populate_with_features(path_buf.clone()).unwrap();
        let repo = CommandRepository::new(
            Box::new(CheckCommand),
            GitPath::CustomDirectory(PathBuf::from(path.path())),
            false,
        );
        match repo.execute(ArgSource::SUPPLIED(vec!["check"])) {
            Ok(_) => {
                panic!("Should fail")
            }
            Err(_) => {
                assert!(true)
            }
        }
    }

    #[test]
    fn check_all() {
        let path = TempDir::new().unwrap();
        let path_buf = PathBuf::from(path.path());
        prepare_empty_git_repo(path_buf.clone()).unwrap();
        populate_with_features(path_buf.clone()).unwrap();
        let repo = CommandRepository::new(
            Box::new(CheckCommand),
            GitPath::CustomDirectory(PathBuf::from(path.path())),
        );
        match repo.execute(ArgSource::SUPPLIED(vec!["check", "--all"])) {
            Ok(statistic) => {
                println!("{:?}", statistic);
                assert!(statistic.contains_log("No conflicts"))
            }
            Err(_) => {
                panic!()
            }
        }
    }

    #[test]
    fn check_current_feature() {
        let path = TempDir::new().unwrap();
        let path_buf = PathBuf::from(path.path());
        prepare_empty_git_repo(path_buf.clone()).unwrap();
        populate_with_features(path_buf.clone()).unwrap();
        GitInterface::new(GitPath::CustomDirectory(path_buf))
            .checkout(&QualifiedPath::from("/main/feature/root/foo"))
            .unwrap();
        let repo = CommandRepository::new(
            Box::new(CheckCommand),
            GitPath::CustomDirectory(PathBuf::from(path.path())),
            true,
        );
        match repo.execute(ArgSource::SUPPLIED(vec!["check", ".", "../bar"])) {
            Ok(statistics) => {
                assert!(statistics.contains_log("/main/root/foo and /main/root/bar OK"));
                assert!(statistics.contains_log("No conflicts"))
            }
            Err(_) => {
                panic!()
            }
        }
    }
}

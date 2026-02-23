use crate::cli::completion::CompletionHelper;
use crate::cli::*;
use crate::git::conflict::{ConflictChecker, ConflictStatistics};
use crate::model::{
    ByQPathFilteringNodePathTransformer, HasBranchFilteringNodePathTransformer,
    NodePathTransformer, NodePathType, QPathFilteringMode, QualifiedPath,
};
use clap::{Arg, ArgAction, Command};
use colored::Colorize;
use std::error::Error;

const SOURCE: &str = "source";
const TARGETS: &str = "targets";
const ALL: &str = "all";

fn run_check(context: &CommandContext) -> Result<ConflictStatistics, Box<dyn Error>> {
    let all = context
        .arg_helper
        .get_argument_value::<bool>(ALL)
        .unwrap_or(false);
    let maybe_feature: Option<QualifiedPath> =
        match context.arg_helper.get_argument_value::<String>(SOURCE) {
            Some(feature) => Some(QualifiedPath::from(feature)),
            None => None,
        };
    let maybe_targets: Option<Vec<QualifiedPath>> =
        match context.arg_helper.get_argument_values::<String>(TARGETS) {
            Some(targets) => Some(targets.into_iter().map(QualifiedPath::from).collect()),
            None => None,
        };
    let feature_root = match context.git.get_current_area()?.to_feature_root() {
        Some(path) => path,
        None => return Err("Nothing to check: no features exist".into()),
    };
    let current_path = context.git.get_current_node_path()?;
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
            let qualified_source = current_path.get_qualified_path() + source;
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
            let qualified_source = current_path.get_qualified_path() + source;
            let qualified_targets: Vec<QualifiedPath> = targets
                .into_iter()
                .map(|target| current_path.get_qualified_path() + QualifiedPath::from(target))
                .collect();
            checker
                .check_1_to_n(&qualified_source, &qualified_targets)?
                .collect()
        }
    };
    Ok(statistics)
}

#[derive(Clone, Debug)]
pub struct CheckCommand;

impl CommandDefinition for CheckCommand {
    fn build_command(&self) -> Command {
        Command::new("check")
            .about("Check features for merge conflicts")
            .disable_help_subcommand(true)
            .arg(
                Arg::new(SOURCE)
                    .default_value(".")
                    .help("Feature to check against targets"),
            )
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
        let statistics = run_check(context)?;
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

    fn shell_complete(
        &self,
        completion_helper: CompletionHelper,
        context: &mut CommandContext,
    ) -> Result<Vec<String>, Box<dyn Error>> {
        let currently_editing = completion_helper.currently_editing();
        let completion: Vec<String> = if currently_editing.is_some() {
            let feature_root = match context.git.get_current_area()?.to_feature_root() {
                Some(path) => path,
                None => return Ok(vec![]),
            };
            let transformer = HasBranchFilteringNodePathTransformer::new(true);
            let relevant_paths = transformer.transform(feature_root.iter_children_req());
            match currently_editing.unwrap().get_id().as_str() {
                SOURCE => completion_helper.complete_qualified_paths(
                    context.git.get_current_qualified_path()?,
                    relevant_paths.map(|path| path.get_qualified_path()),
                ),
                TARGETS => {
                    let current_path = context.git.get_current_qualified_path()?;
                    let to_exclude1 = completion_helper.get_appendix_of(SOURCE);
                    let to_exclude2 = completion_helper.get_appendix_of(TARGETS);
                    let mut to_exclude = vec![];
                    to_exclude.extend(to_exclude1);
                    to_exclude.extend(to_exclude2);
                    let to_exclude_paths = to_exclude
                        .into_iter()
                        .map(|p| current_path.clone() + QualifiedPath::from(p))
                        .collect();
                    let filter = ByQPathFilteringNodePathTransformer::new(
                        to_exclude_paths,
                        QPathFilteringMode::EXCLUDE,
                    );
                    let filtered = filter.transform(relevant_paths);
                    completion_helper.complete_qualified_paths(
                        context.git.get_current_qualified_path()?,
                        filtered.map(|path| path.get_qualified_path()),
                    )
                }
                _ => {
                    vec![]
                }
            }
        } else {
            vec![]
        };
        Ok(completion)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::interface::test_utils::*;
    use crate::git::interface::{GitInterface, GitPath};
    use crate::model::ImportFormat;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn test_check_error_if_not_all_and_no_source() {
        let path = TempDir::new().unwrap();
        let path_buf = PathBuf::from(path.path());
        prepare_empty_git_repo(path_buf.clone()).unwrap();
        populate_with_features(path_buf.clone()).unwrap();
        let repo = CommandRepository::new(
            Box::new(CheckCommand),
            GitPath::CustomDirectory(PathBuf::from(path.path())),
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
    fn test_check_all() {
        let path = TempDir::new().unwrap();
        let path_buf = PathBuf::from(path.path());
        prepare_empty_git_repo(path_buf.clone()).unwrap();
        populate_with_features(path_buf.clone()).unwrap();
        let repo = CommandRepository::new(
            Box::new(CheckCommand),
            GitPath::CustomDirectory(PathBuf::from(path.path())),
        );
        let context = repo.build_context(
            ArgSource::SUPPLIED(vec!["check", "--all"]),
            ImportFormat::Native,
        );
        match run_check(&context) {
            Ok(statistics) => {
                assert_eq!(statistics.n_ok(), 6);
                assert_eq!(statistics.n_conflict(), 0);
                assert_eq!(statistics.n_errors(), 0);
            }
            Err(_) => {
                panic!()
            }
        }
    }

    #[test]
    fn test_check_current_feature() {
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
        );
        let context = repo.build_context(
            ArgSource::SUPPLIED(vec!["check", "."]),
            ImportFormat::Native,
        );
        match run_check(&context) {
            Ok(statistics) => {
                assert_eq!(statistics.n_ok(), 3);
                assert_eq!(statistics.n_conflict(), 0);
                assert_eq!(statistics.n_errors(), 0);
            }
            Err(_) => {
                panic!()
            }
        }
    }

    #[test]
    fn test_check_no_feature() {
        let path = TempDir::new().unwrap();
        let path_buf = PathBuf::from(path.path());
        prepare_empty_git_repo(path_buf.clone()).unwrap();
        populate_with_features(path_buf.clone()).unwrap();
        let repo = CommandRepository::new(
            Box::new(CheckCommand),
            GitPath::CustomDirectory(PathBuf::from(path.path())),
        );
        let context = repo.build_context(
            ArgSource::SUPPLIED(vec!["check", "."]),
            ImportFormat::Native,
        );
        match run_check(&context) {
            Ok(_) => {
                panic!("Should fail")
            }
            Err(_) => {
                assert!(true)
            }
        }
    }

    #[test]
    fn test_check_specific_targets_relative_path() {
        let path = TempDir::new().unwrap();
        let path_buf = PathBuf::from(path.path());
        prepare_empty_git_repo(path_buf.clone()).unwrap();
        populate_with_features(path_buf.clone()).unwrap();
        let repo = CommandRepository::new(
            Box::new(CheckCommand),
            GitPath::CustomDirectory(PathBuf::from(path.path())),
        );
        let context = repo.build_context(
            ArgSource::SUPPLIED(vec![
                "check",
                "feature/root/foo",
                "feature/root/bar",
                "feature/root/baz",
            ]),
            ImportFormat::Native,
        );
        match run_check(&context) {
            Ok(statistics) => {
                assert_eq!(statistics.n_ok(), 2);
                assert_eq!(statistics.n_conflict(), 0);
                assert_eq!(statistics.n_errors(), 0);
            }
            Err(_) => {
                panic!()
            }
        }
    }

    #[test]
    fn test_check_specific_targets_absolute_path() {
        let path = TempDir::new().unwrap();
        let path_buf = PathBuf::from(path.path());
        prepare_empty_git_repo(path_buf.clone()).unwrap();
        populate_with_features(path_buf.clone()).unwrap();
        let repo = CommandRepository::new(
            Box::new(CheckCommand),
            GitPath::CustomDirectory(PathBuf::from(path.path())),
        );
        GitInterface::new(GitPath::CustomDirectory(path_buf))
            .checkout(&QualifiedPath::from("/main/feature/root/foo"))
            .unwrap();
        let context = repo.build_context(
            ArgSource::SUPPLIED(vec![
                "check",
                "/main/feature/root/foo",
                "/main/feature/root/bar",
                "/main/feature/root/baz",
            ]),
            ImportFormat::Native,
        );
        match run_check(&context) {
            Ok(statistics) => {
                assert_eq!(statistics.n_ok(), 2);
                assert_eq!(statistics.n_conflict(), 0);
                assert_eq!(statistics.n_errors(), 0);
            }
            Err(_) => {
                panic!()
            }
        }
    }
}

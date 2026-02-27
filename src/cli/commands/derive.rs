use crate::cli::completion::*;
use crate::cli::*;
use crate::git::conflict::{
    ConflictCheckBaseBranch, ConflictChecker, ConflictStatistic, ConflictStatistics,
};
use crate::model::*;
use clap::{Arg, ArgAction, Command};
use colored::Colorize;
use petgraph::algo::maximal_cliques;
use petgraph::graph::UnGraph;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use uuid::Uuid;

const FEATURES: &str = "features";
const CONTINUE: &str = "continue";
const ABORT: &str = "abort";
const NO_OPTIMIZATION: &str = "no_optimization";
const DERIVATION_COMMENT: &str = "# DO NOT EDIT OR REMOVE THIS COMMIT\nDERIVATION STATUS\n";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureMetadata {
    path: String,
}
impl FeatureMetadata {
    pub fn new<S: Into<String>>(path: S) -> Self {
        Self { path: path.into() }
    }
    pub fn get_qualified_path(&self) -> QualifiedPath {
        QualifiedPath::from(&self.path)
    }
}

pub enum DerivationState {
    Starting,
    InProgress,
    Finished,
}
impl Display for DerivationState {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let out = match self {
            DerivationState::Starting => "starting",
            DerivationState::InProgress => "in_progress",
            DerivationState::Finished => "finished",
        };
        f.write_str(out)
    }
}
impl DerivationState {
    pub fn from_string<S: Into<String>>(from: S) -> Self {
        let real = from.into();
        if real == "starting" {
            Self::Starting
        } else if real == "in_progress" {
            Self::InProgress
        } else if real == "finished" {
            Self::Finished
        } else {
            unreachable!()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DerivationMetadata {
    id: String,
    state: String,
    initial_commit: String,
    completed: Vec<FeatureMetadata>,
    missing: Vec<FeatureMetadata>,
    total: Vec<FeatureMetadata>,
}
impl DerivationMetadata {
    fn new<S: Into<String>>(
        id: S,
        state: DerivationState,
        initial_commit: S,
        completed: Vec<FeatureMetadata>,
        missing: Vec<FeatureMetadata>,
        total: Vec<FeatureMetadata>,
    ) -> Self {
        Self {
            id: id.into(),
            initial_commit: initial_commit.into(),
            state: state.to_string(),
            completed,
            missing,
            total,
        }
    }
    pub fn new_initial<S: Into<String>>(features: Vec<FeatureMetadata>, initial_commit: S) -> Self {
        let uuid = Uuid::new_v4();
        Self::new(
            uuid.to_string(),
            DerivationState::Starting,
            initial_commit.into(),
            Vec::new(),
            features.clone(),
            features,
        )
    }
    pub fn new_from_previously_finished<S: Into<String>>(
        previous: &Self,
        features: Vec<FeatureMetadata>,
        starting_commit: S,
    ) -> Self {
        match previous.get_state() {
            DerivationState::Finished => {}
            _ => panic!("Unexpected derivation state {}", previous.get_state()),
        }
        let uuid = Uuid::new_v4();
        let mut total = previous.get_total().clone();
        for feature in features.clone() {
            if !total.contains(&feature) {
                total.push(feature);
            }
        }
        Self::new(
            uuid.to_string(),
            DerivationState::Starting,
            starting_commit.into(),
            Vec::new(),
            features,
            total,
        )
    }
    pub fn as_finished(&mut self) {
        self.state = DerivationState::Finished.to_string();
    }
    pub fn as_in_progress(&mut self) {
        self.state = DerivationState::InProgress.to_string();
    }
    pub fn mark_as_completed(&mut self, features: &Vec<QualifiedPath>) {
        for feature in features {
            let old_missing: Vec<FeatureMetadata> = self.missing.clone();
            let missing = old_missing
                .iter()
                .find(|m| m.get_qualified_path() == *feature);
            if missing.is_some() {
                self.missing.retain(|m| m.get_qualified_path() != *feature);
                self.completed.push(missing.unwrap().clone())
            }
        }
    }
    pub fn get_completed(&self) -> &Vec<FeatureMetadata> {
        &self.completed
    }
    pub fn get_missing(&self) -> &Vec<FeatureMetadata> {
        &self.missing
    }
    pub fn get_total(&self) -> &Vec<FeatureMetadata> {
        &self.total
    }
    pub fn get_state(&self) -> DerivationState {
        DerivationState::from_string(&self.state)
    }
    pub fn get_id(&self) -> &String {
        &self.id
    }
    pub fn get_initial_commit(&self) -> &String {
        &self.initial_commit
    }
}

fn make_derivation_commit_message(
    derivation_metadata: &DerivationMetadata,
) -> serde_json::error::Result<String> {
    let base = DERIVATION_COMMENT.to_string();
    let serialized = serde_json::to_string(&derivation_metadata)?;
    Ok(base + serialized.as_str())
}
pub fn parse_derivation_commit_message(
    commit: &Commit,
) -> Option<serde_json::error::Result<DerivationMetadata>> {
    if !commit.get_message().contains(DERIVATION_COMMENT) {
        return None;
    }
    let formatted = commit.get_message().replace(DERIVATION_COMMENT, "");
    match serde_json::from_str::<DerivationMetadata>(&formatted) {
        Ok(result) => Some(Ok(result)),
        Err(e) => Some(Err(e)),
    }
}

fn map_paths_to_id(
    paths: &Vec<QualifiedPath>,
) -> (HashMap<usize, QualifiedPath>, HashMap<QualifiedPath, usize>) {
    let mut id_to_path: HashMap<usize, QualifiedPath> = HashMap::new();
    let mut path_to_id: HashMap<QualifiedPath, usize> = HashMap::new();
    let mut i = 0;
    for path in paths.iter() {
        id_to_path.insert(i, path.clone());
        path_to_id.insert(path.clone(), i);
        i += 1;
    }
    (id_to_path, path_to_id)
}

fn build_edges(
    conflict_data: &ConflictStatistics,
    path_to_id: &HashMap<QualifiedPath, usize>,
) -> Vec<(u32, u32)> {
    conflict_data
        .iter_ok()
        .map(|element| match element {
            ConflictStatistic::OK((l, r)) => {
                let left = path_to_id.get(l).unwrap().clone() as u32;
                let right = path_to_id.get(r).unwrap().clone() as u32;
                (left, right)
            }
            _ => unreachable!(),
        })
        .collect()
}

fn get_max_clique(graph: &UnGraph<usize, ()>) -> Vec<usize> {
    let cliques = maximal_cliques(graph);
    let mut max_clique: Vec<usize> = Vec::new();
    for clique in cliques.iter() {
        if clique.len() > max_clique.len() {
            max_clique = clique.iter().map(|e| e.index()).collect();
        }
    }
    max_clique
}

fn clique_to_paths(
    clique: Vec<usize>,
    id_to_path: &HashMap<usize, QualifiedPath>,
) -> Vec<QualifiedPath> {
    let mut paths: Vec<QualifiedPath> = Vec::new();
    for path in clique {
        paths.push(id_to_path.get(&path).unwrap().clone());
    }
    paths
}

fn get_last_metadata(commits: &Vec<Commit>) -> Result<Option<DerivationMetadata>, Box<dyn Error>> {
    let last_state =
        commits
            .iter()
            .find_map(|commit| match parse_derivation_commit_message(commit) {
                Some(result) => Some(result),
                None => None,
            });
    match last_state {
        Some(last_state) => Ok(Some(last_state?)),
        None => Ok(None),
    }
}

fn handle_abort(
    last_state: &Option<DerivationMetadata>,
    abort: bool,
    context: &CommandContext,
) -> Result<bool, Box<dyn Error>> {
    match (last_state, abort) {
        (None, true) => Err("Derivation not started, there is nothing to abort".into()),
        (Some(last_state), true) => match last_state.get_state() {
            DerivationState::Finished => {
                Err("Derivation finished, there is nothing to abort".into())
            }
            _ => {
                context.info("Aborting current derivation process");
                let commit = last_state.get_initial_commit();
                context.git.abort_merge()?;
                context.git.reset_hard(commit)?;
                context.info(format!("Reset to state before derivation ({})", commit));
                Ok(true)
            }
        },
        (_, false) => Ok(false),
    }
}

fn handle_continue(
    last_state: &Option<DerivationMetadata>,
    continue_derivation: bool,
    context: &mut CommandContext,
) -> Result<bool, Box<dyn Error>> {
    match (last_state, continue_derivation) {
        (None, true) => Err("Derivation not started, there is nothing to continue".into()),
        (Some(last_state), true) => match last_state.get_state() {
            DerivationState::Finished => {
                Err("Derivation finished, there is nothing to continue".into())
            }
            _ => {
                let missing = last_state
                    .get_missing()
                    .iter()
                    .map(|m| m.get_qualified_path())
                    .collect::<Vec<QualifiedPath>>();
                let mergeable = calculate_features_without_conflicts(&missing, context)?;
                handle_derivation(&mergeable, last_state.clone(), context)?;
                Ok(true)
            }
        },
        (Some(last_state), false) => match last_state.get_state() {
            DerivationState::Starting | DerivationState::InProgress => Err(format!(
                "Derivation incomplete, please use {} to finish it first",
                "tangl derive --continue".yellow()
            )
            .into()),
            _ => Ok(false),
        },
        (_, false) => Ok(false),
    }
}

fn handle_derivation(
    mut progress: DerivationMetadata,
    no_optimization: bool,
    context: &mut CommandContext,
) -> Result<(), Box<dyn Error>> {
    let missing = progress
        .get_missing()
        .iter()
        .map(|m| m.get_qualified_path())
        .collect::<Vec<QualifiedPath>>();
    let merge_order: Vec<QualifiedPath> = match no_optimization {
        false => {
            let likely_mergeable = calculate_features_without_conflicts(&missing, context)?;
            let mut cloned = likely_mergeable.clone();
            cloned.extend(
                missing
                    .iter()
                    .filter(|m| !likely_mergeable.contains(m))
                    .cloned(),
            );
            cloned
        }
        true => {
            context.info("Info: merge order optimization is disabled".yellow().to_string());
            missing
        },
    };
    let mut completed: Vec<QualifiedPath> = Vec::new();
    for path in merge_order {
        let path_vec = vec![path.clone()];
        let result = context.git.merge(&path_vec)?;
        match result.status.success() {
            true => {
                progress.mark_as_completed(&path_vec);
                completed.push(path)
            },
            false => {
                context.git.abort_merge()?;
                break
            },
        }
    }
    context.info(format!(
        "{} features where merged {}:\n",
        merge_order.len().to_string().green(),
        "successfully".green(),
    ));
    for path in merge_order.iter() {
        context.info(format!("  {}", path.to_string().green()))
    }
    match progress.get_missing().len() {
        0 => {
            progress.as_finished();
            context
                .git
                .empty_commit(make_derivation_commit_message(&progress)?.as_str())?;
            context.info(format!(
                "\nNo missing features remain. Derivation {}.:\n",
                "complete".green(),
            ));
        }
        _ => {
            progress.as_in_progress();
            context
                .git
                .empty_commit(make_derivation_commit_message(&progress)?.as_str())?;
            context.info(format!(
                "\n{} {} feature(s) remain(s):\n",
                progress.get_missing().len().to_string().red(),
                "conflicting".red()
            ));
            for data in progress.get_missing().iter() {
                context.info(format!("  {}", data.get_qualified_path().to_string().red()))
            }
            let to_merge = progress.get_missing()[0].get_qualified_path();
            context.info(format!(
                "\nNow merging:\n\n   {}",
                to_merge.to_string().yellow(),
            ));
            context.git.merge(&vec![to_merge])?;
            context.info(format!(
                "\nPlease solve all conflicts and commit your changes. Thereafter, run {} to continue the derivation.",
                "tangl derive --continue".purple()
            ));
            context.info(format!(
                "Use {} to abort the current derivation process.",
                "tangl derive --abort".purple()
            ));
        }
    }
    Ok(())
}

fn calculate_features_without_conflicts(
    features: &Vec<QualifiedPath>,
    context: &CommandContext,
) -> Result<Vec<QualifiedPath>, Box<dyn Error>> {
    let (id_to_path, path_to_id) = map_paths_to_id(features);
    let conflicts: ConflictStatistics =
        ConflictChecker::new(&context.git, ConflictCheckBaseBranch::Current)
            .check_n_to_n_pairwise(features)?
            .collect();
    if conflicts.n_errors() > 0 {
        return Err("Errors occurred while checking for conflicts.".into());
    }
    let edges = build_edges(&conflicts, &path_to_id);
    let graph = UnGraph::<usize, ()>::from_edges(&edges);
    let max_clique = get_max_clique(&graph);
    Ok(clique_to_paths(max_clique, &id_to_path))
}

#[derive(Clone, Debug)]
pub struct DeriveCommand;

impl CommandDefinition for DeriveCommand {
    fn build_command(&self) -> Command {
        Command::new("derive")
            .about("Derive a product")
            .disable_help_subcommand(true)
            .arg(Arg::new(FEATURES).action(ArgAction::Append))
            .arg(
                Arg::new(CONTINUE)
                    .long("continue")
                    .action(ArgAction::SetTrue)
                    .help("Continue the ongoing derivation process"),
            )
            .arg(
                Arg::new(ABORT)
                    .long(ABORT)
                    .action(ArgAction::SetTrue)
                    .help("Abort the ongoing derivation process"),
            )
            .arg(
                Arg::new(NO_OPTIMIZATION)
                    .long("no-optimization")
                    .action(ArgAction::SetTrue)
                    .help("Disable optimization of merge order"),
            )
    }
}

impl CommandInterface for DeriveCommand {
    fn run_command(&self, context: &mut CommandContext) -> Result<(), Box<dyn Error>> {
        let current_area = context.git.get_current_area()?;
        let current_path = context.git.get_current_node_path()?;
        let product_path = match current_path.concretize() {
            NodePathType::Product(path) => path.get_qualified_path(),
            _ => {
                return Err(format!(
                    "Current branch is not a product. You can create one with the {} command and/or {} one.",
                    "product".italic().bold(),
                    "checkout".italic().bold(),
                )
                .into());
            }
        };
        let continue_derivation = context
            .arg_helper
            .get_argument_value::<bool>(CONTINUE)
            .unwrap();
        let abort_derivation = context
            .arg_helper
            .get_argument_value::<bool>(ABORT)
            .unwrap();
        let no_optimization = context
            .arg_helper
            .get_argument_value::<bool>(NO_OPTIMIZATION)
            .unwrap();

        let commits = context.git.get_commit_history(&product_path)?;
        let last_state = get_last_metadata(&commits)?;

        // handle abort flag
        if handle_abort(&last_state, abort_derivation, context)? {
            return Ok(());
        }
        // handle continue flag
        if handle_continue(&last_state, continue_derivation, context)? {
            return Ok(());
        }
        // now we know, this derivation is the initial one,
        // regardless if there are previous succeeded ones or not

        let all_features = context
            .arg_helper
            .get_argument_values::<String>(FEATURES)
            .unwrap()
            .into_iter()
            .map(|e| current_area.get_path_to_feature_root() + QualifiedPath::from(e))
            .collect::<Vec<_>>();
        drop(current_area);
        let features_metadata: Vec<FeatureMetadata> = all_features
            .iter()
            .map(|f| FeatureMetadata::new(f.clone()))
            .collect();
        let initial_metadata = match last_state {
            Some(state) => match state.get_state() {
                DerivationState::Finished => DerivationMetadata::new_from_previously_finished(
                    &state,
                    features_metadata,
                    commits[0].get_hash(),
                ),
                _ => panic!("Unexpected derivation state {}", state.get_state()),
            },
            None => DerivationMetadata::new_initial(features_metadata, commits[0].get_hash()),
        };
        context
            .git
            .empty_commit(make_derivation_commit_message(&initial_metadata)?.as_str())?;
        let missing_features: Vec<QualifiedPath> = initial_metadata
            .get_missing()
            .iter()
            .map(|m| m.get_qualified_path())
            .collect();

        let mergeable_features = calculate_features_without_conflicts(&missing_features, &context)?;
        handle_derivation(initial_metadata, no_optimization, context)?;
        Ok(())
    }
    fn shell_complete(
        &self,
        completion_helper: CompletionHelper,
        context: &mut CommandContext,
    ) -> Result<Vec<String>, Box<dyn Error>> {
        let maybe_feature_root = context.git.get_current_area()?.to_feature_root();
        if maybe_feature_root.is_none() {
            return Ok(vec![]);
        }
        let feature_root = maybe_feature_root.unwrap();
        let feature_root_path = feature_root.get_qualified_path();
        let current = completion_helper.currently_editing();
        let result = match current {
            Some(value) => match value.get_id().as_str() {
                FEATURES => {
                    let to_filter = completion_helper
                        .get_appendix_of(FEATURES)
                        .into_iter()
                        .map(|p| feature_root_path.clone() + QualifiedPath::from(p))
                        .collect();
                    let transformer = ChainingNodePathTransformer::new(vec![
                        NodePathTransformers::HasBranchFilteringNodePathTransformer(
                            HasBranchFilteringNodePathTransformer::new(true),
                        ),
                        NodePathTransformers::ByQPathFilteringNodePathTransformer(
                            ByQPathFilteringNodePathTransformer::new(
                                to_filter,
                                QPathFilteringMode::EXCLUDE,
                            ),
                        ),
                    ]);
                    completion_helper.complete_qualified_paths(
                        feature_root.get_qualified_path(),
                        transformer
                            .transform(feature_root.iter_children_req())
                            .map(|path| path.get_qualified_path()),
                    )
                }
                _ => vec![],
            },
            None => vec![],
        };
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::interface::test_utils::{populate_with_features, prepare_empty_git_repo};
    use crate::git::interface::{GitInterface, GitPath};
    use crate::model::NodePathProductNavigation;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn test_derivation_commit_message() {
        let origin_metadata = DerivationMetadata::new(
            vec![FeatureMetadata::new("/main/feature/root/foo")],
            vec![FeatureMetadata::new("/main/feature/root/bar")],
        );
        let written = make_derivation_commit_message(&origin_metadata).unwrap();
        let commit = Commit::new("hash", written);
        let parsed = parse_derivation_commit_message(&commit).unwrap().unwrap();
        assert_eq!(origin_metadata, parsed);
    }

    #[test]
    fn test_derivation_commit_message_parse_wrong_commit() {
        let commit = Commit::new("hash", "foo");
        let parsed = parse_derivation_commit_message(&commit);
        match parsed {
            Some(_) => panic!("parse should not be ok"),
            None => assert!(true),
        }
    }

    #[test]
    fn test_derivation_no_conflicts() {
        let path = TempDir::new().unwrap();
        prepare_empty_git_repo(PathBuf::from(path.path())).unwrap();
        populate_with_features(PathBuf::from(path.path())).unwrap();
        let repo = CommandRepository::new(
            Box::new(DeriveCommand),
            GitPath::CustomDirectory(PathBuf::from(path.path())),
        );
        match repo.execute(ArgSource::SUPPLIED(vec![
            "derive", "-p", "myprod", "root/foo", "root/bar", "root/baz",
        ])) {
            Ok(_) => {
                let interface = GitInterface::in_directory(PathBuf::from(path.path()));
                interface
                    .get_current_area()
                    .unwrap()
                    .to_product_root()
                    .unwrap()
                    .to_product(&QualifiedPath::from("myprod"))
                    .unwrap();
            }
            Err(e) => panic!("{}", e),
        }
    }

    #[test]
    fn test_derivation_commit() {
        let path = TempDir::new().unwrap();
        prepare_empty_git_repo(PathBuf::from(path.path())).unwrap();
        populate_with_features(PathBuf::from(path.path())).unwrap();
        let repo = CommandRepository::new(
            Box::new(DeriveCommand),
            GitPath::CustomDirectory(PathBuf::from(path.path())),
        );
        match repo.execute(ArgSource::SUPPLIED(vec![
            "derive", "-p", "myprod", "root/foo", "root/bar", "root/baz",
        ])) {
            Ok(_) => {
                let interface = GitInterface::in_directory(PathBuf::from(path.path()));
                let product = interface
                    .get_current_area()
                    .unwrap()
                    .to_product_root()
                    .unwrap()
                    .to_product(&QualifiedPath::from("myprod"))
                    .unwrap();
                let commits = interface
                    .get_commit_history(&product.get_qualified_path())
                    .unwrap();
                let derivation_commit = commits[0].clone();
                assert_eq!(
                    derivation_commit.get_message(),
                    &make_post_derivation_message(&vec![
                        QualifiedPath::from("/main/feature/root/foo"),
                        QualifiedPath::from("/main/feature/root/bar"),
                        QualifiedPath::from("/main/feature/root/baz"),
                    ]),
                )
            }
            Err(e) => panic!("{}", e),
        }
    }
}

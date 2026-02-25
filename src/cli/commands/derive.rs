use crate::cli::completion::*;
use crate::cli::*;
use crate::git::conflict::{ConflictChecker, ConflictStatistic, ConflictStatistics};
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
const ALLOW_STEPWISE_DERIVATION: &str = "allow_stepwise_derivation";
const CONTINUE: &str = "continue";
const ABORT: &str = "abort";
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
    completed: Vec<FeatureMetadata>,
    missing: Vec<FeatureMetadata>,
    total: Vec<FeatureMetadata>,
}
impl DerivationMetadata {
    fn new<S: Into<String>>(
        id: S,
        state: DerivationState,
        completed: Vec<FeatureMetadata>,
        missing: Vec<FeatureMetadata>,
        total: Vec<FeatureMetadata>,
    ) -> Self {
        Self {
            id: id.into(),
            state: state.to_string(),
            completed,
            missing,
            total,
        }
    }
    pub fn new_initial(features: Vec<FeatureMetadata>) -> Self {
        let uuid = Uuid::new_v4();
        Self::new(
            uuid.to_string(),
            DerivationState::Starting,
            Vec::new(),
            features.clone(),
            features,
        )
    }
    pub fn new_from_previously_finished(previous: &Self, features: Vec<FeatureMetadata>) -> Self {
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

fn get_derivation_start_metadata(
    id: &str,
    commits: &Vec<Commit>,
) -> Result<Option<(DerivationMetadata, usize)>, Box<dyn Error>> {
    let mut searched: Option<DerivationMetadata> = None;
    let mut i: usize = 0;
    for (index, commit) in commits.iter().enumerate() {
        let parsed = parse_derivation_commit_message(commit);
        match parsed {
            Some(result) => {
                let unpacked = result?;
                if unpacked.get_id() == id {
                    match unpacked.get_state() {
                        DerivationState::Starting => {
                            i = index;
                            searched = Some(unpacked);
                        }
                        _ => {}
                    }
                }
            }
            None => {}
        }
    }
    if searched.is_some() {
        Ok(Some((searched.unwrap(), i)))
    } else {
        Ok(None)
    }
}

fn handle_abort(
    last_state: &Option<DerivationMetadata>,
    commits: &Vec<Commit>,
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
                let (_, index) =
                    get_derivation_start_metadata(last_state.get_id(), commits)?.unwrap();
                let commit = commits.get(index+1).unwrap();
                context.git.reset_hard(commit.get_hash())?;
                context.info(format!("Reset to last clean state ({})", commit.get_hash()));
                Ok(true)
            }
        },
        (_, false) => Ok(false),
    }
}

fn handle_continue(
    last_state: &Option<DerivationMetadata>,
    continue_derivation: bool,
    context: &CommandContext,
) -> Result<bool, Box<dyn Error>> {
    match (last_state, continue_derivation) {
        (None, true) => Err("Derivation not started, there is nothing to continue".into()),
        (Some(last_state), true) => {
            match last_state.get_state() {
                DerivationState::Finished => {
                    Err("Derivation finished, there is nothing to continue".into())
                }
                _ => {
                    let feature_data: &FeatureMetadata = &last_state.get_missing()[0];
                    context.info(format!(
                        "Merging conflicting feature {}",
                        feature_data.get_qualified_path().to_string().red()
                    ));
                    // TODO
                    context.info(format!(
                        "Please solve all conflicts and commit your changes. Thereafter, run {}",
                        "tangl derive --continue".italic().bold()
                    ));
                    Ok(true)
                }
            }
        }
        (Some(last_state), false) => match last_state.get_state() {
            DerivationState::Starting | DerivationState::InProgress => Err(format!(
                "Derivation incomplete, please use {} to finish it first",
                "--continue".italic().bold()
            )
            .into()),
            _ => Ok(false),
        },
        (_, false) => Ok(false),
    }
}

fn handle_full_derivation(
    features: &Vec<QualifiedPath>,
    metadata: DerivationMetadata,
    context: &mut CommandContext,
) -> Result<(), Box<dyn Error>> {
    let mut finished = derivation_without_conflicts(features, metadata, context)?;
    finished.as_finished();
    context
        .git
        .empty_commit(make_derivation_commit_message(&finished)?.as_str())?;
    context.info(format!(
        "Derivation finished {}",
        "without conflicts".green()
    ));
    Ok(())
}

fn handle_partial_derivation(
    stepwise: bool,
    mergeable: &Vec<QualifiedPath>,
    missing: &Vec<QualifiedPath>,
    metadata: DerivationMetadata,
    context: &mut CommandContext,
) -> Result<(), Box<dyn Error>> {
    match stepwise {
        true => {
            let mut progress = derivation_without_conflicts(mergeable, metadata, context)?;
            progress.as_in_progress();
            context
                .git
                .empty_commit(make_derivation_commit_message(&progress)?.as_str())?;
            context.info(format!(
                "Merged {} features, while {} are still missing:\n",
                mergeable.len().to_string().green(),
                missing.len().to_string().red()
            ));
            for path in missing.iter() {
                context.info(format!("  {}", path.to_string().red()))
            }
            context.info(
                "\nUse --continue to merge missing features step-wise and solve their conflicts",
            );
        }
        false => {
            context.info(format!(
                "Can merge {} feature(s) {}:\n",
                mergeable.len().to_string().green(),
                "without conflicts".green()
            ));
            for path in mergeable.iter() {
                context.info(format!("  {}", path.to_string().green()));
            }
            context.info(format!(
                "\n{} feature(s) {}:\n",
                missing.len().to_string().red(),
                "will produce conflicts".red()
            ));
            for path in missing.iter() {
                context.info(format!("  {}", path.to_string().red()));
            }
            context.info(
                "\nHint: Use the --allow-stepwise-derivation \
                    to merge all conflict-free features \
                    and to perform a step-wise derivation to solve all conflicts manually.",
            );
        }
    }
    Ok(())
}

fn calculate_mergeable_features(
    features: &Vec<QualifiedPath>,
    context: &CommandContext,
) -> Result<Vec<QualifiedPath>, Box<dyn Error>> {
    let (id_to_path, path_to_id) = map_paths_to_id(features);
    let conflicts: ConflictStatistics = ConflictChecker::new(&context.git)
        .check_all(features)?
        .collect();
    if conflicts.n_errors() > 0 {
        return Err("Errors occurred while checking for conflicts.".into());
    }
    let edges = build_edges(&conflicts, &path_to_id);
    let graph = UnGraph::<usize, ()>::from_edges(&edges);
    let max_clique = get_max_clique(&graph);
    Ok(clique_to_paths(max_clique, &id_to_path))
}

fn derivation_without_conflicts(
    features: &Vec<QualifiedPath>,
    mut metadata: DerivationMetadata,
    context: &mut CommandContext,
) -> Result<DerivationMetadata, Box<dyn Error>> {
    context.git.merge(features)?;
    metadata.mark_as_completed(features);
    Ok(metadata)
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
                Arg::new(ALLOW_STEPWISE_DERIVATION)
                    .long("allow-partial-derivation")
                    .action(ArgAction::SetTrue)
                    .long_help("TODO"),
            )
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
        let allow_stepwise_derivation = context
            .arg_helper
            .get_argument_value::<bool>(ALLOW_STEPWISE_DERIVATION)
            .unwrap();
        let continue_derivation = context
            .arg_helper
            .get_argument_value::<bool>(CONTINUE)
            .unwrap();
        let abort_derivation = context
            .arg_helper
            .get_argument_value::<bool>(ABORT)
            .unwrap();

        let commits = context.git.get_commit_history(&product_path)?;
        let last_state = get_last_metadata(&commits)?;

        // handle abort flag
        if handle_abort(&last_state, &commits, abort_derivation, context)? {
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
                DerivationState::Finished => {
                    DerivationMetadata::new_from_previously_finished(&state, features_metadata)
                }
                _ => panic!("Unexpected derivation state {}", state.get_state()),
            },
            None => DerivationMetadata::new_initial(features_metadata),
        };
        context
            .git
            .empty_commit(make_derivation_commit_message(&initial_metadata)?.as_str())?;

        let mergeable_features = calculate_mergeable_features(&all_features, &context)?;

        // no conflicts
        if mergeable_features.len() == all_features.len() {
            handle_full_derivation(&mergeable_features, initial_metadata, context)?;
        }
        // conflicts
        else {
            let missing: Vec<QualifiedPath> = all_features
                .into_iter()
                .filter(|path| !mergeable_features.contains(path))
                .collect();
            handle_partial_derivation(
                allow_stepwise_derivation,
                &mergeable_features,
                &missing,
                initial_metadata,
                context,
            )?;
        }
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

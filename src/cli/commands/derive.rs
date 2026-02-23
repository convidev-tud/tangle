use crate::cli::completion::*;
use crate::cli::*;
use crate::git::conflict::{ConflictChecker, ConflictStatistic, ConflictStatistics};
use crate::model::{
    ByQPathFilteringNodePathTransformer, ChainingNodePathTransformer, Commit,
    HasBranchFilteringNodePathTransformer, NodePathTransformer, NodePathTransformers,
    QPathFilteringMode, QualifiedPath,
};
use clap::{Arg, ArgAction, Command};
use colored::Colorize;
use petgraph::algo::maximal_cliques;
use petgraph::graph::UnGraph;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;

const FEATURES: &str = "features";
const PRODUCT: &str = "product";
const ALLOW_STEPWISE_DERIVATION: &str = "allow_stepwise_derivation";
const CONTINUE: &str = "continue";
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DerivationMetadata {
    completed: Vec<FeatureMetadata>,
    missing: Vec<FeatureMetadata>,
}
impl DerivationMetadata {
    pub fn new(completed: Vec<FeatureMetadata>, missing: Vec<FeatureMetadata>) -> Self {
        Self { completed, missing }
    }
    pub fn empty() -> Self {
        Self::new(Vec::new(), Vec::new())
    }
    pub fn add_completed(&mut self, metadata: FeatureMetadata) {
        self.completed.push(metadata);
    }
    pub fn add_missing(&mut self, metadata: FeatureMetadata) {
        self.missing.push(metadata);
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
    missing: &Vec<QualifiedPath>,
    product: &QualifiedPath,
    context: &mut CommandContext,
) -> Result<(), Box<dyn Error>> {
    let area = context.git.get_current_area()?;
    let area_path = area.get_qualified_path();
    drop(area);
    context.git.checkout(&area_path)?;
    context.git.create_branch(product)?;
    context.git.checkout(product)?;
    context.git.merge(features)?;
    let derived_features: Vec<FeatureMetadata> = features
        .iter()
        .map(|f| FeatureMetadata::new(f.to_string()))
        .collect();
    let missing_features: Vec<FeatureMetadata> = missing
        .iter()
        .map(|f| FeatureMetadata::new(f.to_string()))
        .collect();
    let derivation_metadata = DerivationMetadata::new(derived_features, missing_features);
    context
        .git
        .empty_commit(make_derivation_commit_message(&derivation_metadata)?.as_str())?;
    Ok(())
}

#[derive(Clone, Debug)]
pub struct DeriveCommand;

impl CommandDefinition for DeriveCommand {
    fn build_command(&self) -> Command {
        Command::new("derive")
            .about("Derive a product")
            .disable_help_subcommand(true)
            .arg(Arg::new(FEATURES).action(ArgAction::Append).required(true))
            .arg(
                Arg::new(PRODUCT)
                    .short('p')
                    .help("Specifies the name of the resulting product branch"),
            )
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
    }
}

impl CommandInterface for DeriveCommand {
    fn run_command(&self, context: &mut CommandContext) -> Result<(), Box<dyn Error>> {
        let target_product_name = context
            .arg_helper
            .get_argument_value::<String>(PRODUCT)
            .unwrap();
        let allow_partial_derivation = context
            .arg_helper
            .get_argument_value::<bool>(ALLOW_STEPWISE_DERIVATION)
            .unwrap();
        let continue_derivation = context
            .arg_helper
            .get_argument_value::<bool>(CONTINUE)
            .unwrap();
        let current_path = context.git.get_current_qualified_path()?;
        let current_area = context.git.get_current_area()?;
        let target_path =
            current_area.get_path_to_product_root() + QualifiedPath::from(target_product_name);

        let all_features = context
            .arg_helper
            .get_argument_values::<String>(FEATURES)
            .unwrap()
            .into_iter()
            .map(|e| current_area.get_path_to_feature_root() + QualifiedPath::from(e))
            .collect::<Vec<_>>();

        context.info("Checking for conflicts");
        let mergeable_features = calculate_mergeable_features(&all_features, &context)?;

        drop(current_area);
        // no conflicts
        if mergeable_features.len() == all_features.len() {
            derivation_without_conflicts(&mergeable_features, &vec![], &target_path, context)?;
            context.git.checkout(&current_path)?;
            context.info(
                "Derivation finished ".to_string()
                    + "without conflicts".green().to_string().as_str(),
            );
        }
        // conflicts
        else {
            let missing: Vec<QualifiedPath> = all_features
                .into_iter()
                .filter(|path| !mergeable_features.contains(path))
                .collect();
            if allow_partial_derivation {
                derivation_without_conflicts(&mergeable_features, &missing, &target_path, context)?;
                context.info(format!(
                    "Merged {} features, while {} are still missing:\n",
                    mergeable_features.len().to_string().green(),
                    missing.len().to_string().red()
                ));
                for path in missing.iter() {
                    context.info(format!("  {}", path.to_string().red()))
                }
                context.info(
                    "\nUse --continue to merge missing features step-wise and solve their conflicts",
                );
            } else {
                context.info(format!(
                    "Can merge {} feature(s) {}:\n",
                    mergeable_features.len().to_string().green(),
                    "without conflicts".green()
                ));
                for path in mergeable_features.iter() {
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
                )
            }
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

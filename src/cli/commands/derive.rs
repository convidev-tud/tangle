use crate::cli::completion::*;
use crate::cli::*;
use crate::git::conflict::{ConflictChecker, ConflictStatistic, ConflictStatistics};
use crate::model::{
    ByQPathFilteringNodePathTransformer, ChainingNodePathTransformer,
    HasBranchFilteringNodePathTransformer, NodePathTransformer, NodePathTransformers,
    QPathFilteringMode, QualifiedPath,
};
use clap::{Arg, ArgAction, Command};
use colored::Colorize;
use petgraph::algo::maximal_cliques;
use petgraph::graph::UnGraph;
use std::collections::HashMap;
use std::error::Error;

const FEATURES: &str = "features";

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

fn make_post_derivation_message(features: &Vec<QualifiedPath>) -> String {
    let mut base = "# DO NOT EDIT OR REMOVE THIS COMMIT\nDERIVATION FINISHED\n".to_string();
    let strings = features
        .iter()
        .map(|f| f.to_string())
        .collect::<Vec<String>>();
    base.push_str(strings.join("\n").as_str());
    base
}

fn make_no_conflict_log() -> String {
    "without conflicts".green().to_string()
}

fn make_conflict_log() -> String {
    "will produce conflicts".red().to_string()
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
                Arg::new("product")
                    .short('p')
                    .required(true)
                    .help("Specifies the name of the resulting product branch"),
            )
    }
}

impl CommandInterface for DeriveCommand {
    fn run_command(&self, context: &mut CommandContext) -> Result<(), Box<dyn Error>> {
        let target_product_name = context
            .arg_helper
            .get_argument_value::<String>("product")
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
        let (id_to_path, path_to_id) = map_paths_to_id(&all_features);
        let conflicts: ConflictStatistics = ConflictChecker::new(&context.git)
            .check_all(&all_features)?
            .collect();
        if conflicts.n_errors() > 0 {
            return Err("Errors occurred while checking for conflicts.".into());
        }
        let edges = build_edges(&conflicts, &path_to_id);
        let graph = UnGraph::<usize, ()>::from_edges(&edges);
        let max_clique = get_max_clique(&graph);
        let mergeable_features = clique_to_paths(max_clique, &id_to_path);
        if mergeable_features.len() == all_features.len() {
            let area_path = current_area.get_qualified_path();
            drop(current_area);
            context.git.checkout(&area_path)?;
            context.git.create_branch(&target_path)?;
            context.git.checkout(&target_path)?;
            context.git.merge(&all_features)?;
            context
                .git
                .empty_commit(make_post_derivation_message(&all_features).as_str())?;
            context.git.checkout(&current_path)?;
            context
                .info("Derivation finished ".to_string() + make_no_conflict_log().as_str() + ".");
        } else {
            context.info(
                format!("Can merge {} features ", mergeable_features.len())
                    + make_no_conflict_log().as_str()
                    + ".",
            );
            context.info(
                format!(
                    "{} features ",
                    all_features.len() - mergeable_features.len()
                ) + make_conflict_log().as_str()
                    + ".",
            );
            context.info("A partial derivation will be performed with all conflict-free features.")
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
    fn derivation_no_conflicts() {
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
    fn derivation_commit() {
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
                    derivation_commit.message(),
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

use crate::cli::CommandContext;
use crate::git::error::GitError;
use crate::git::interface::GitInterface;
use crate::model::QualifiedPath;
use colored::Colorize;
use itertools::Itertools;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeSuccess {
    pub paths: Vec<QualifiedPath>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeConflict {
    pub paths: Vec<QualifiedPath>,
    pub failed_at: Vec<usize>,
    pub tested: Vec<usize>,
}

#[derive(Debug)]
pub struct MergeError {
    pub paths: Vec<QualifiedPath>,
    pub error: GitError,
}
impl PartialEq for MergeError {
    fn eq(&self, other: &Self) -> bool {
        other.paths == self.paths
    }
}

#[derive(Debug)]
pub enum ConflictStatistic {
    Success(MergeSuccess),
    Conflict(MergeConflict),
    Error(MergeError),
}

impl PartialEq for ConflictStatistic {
    fn eq(&self, other: &Self) -> bool {
        match other {
            Self::Success(other_paths) => match self {
                Self::Success(self_paths) => other_paths == self_paths,
                _ => false,
            },
            Self::Conflict(other_paths) => match self {
                Self::Conflict(self_paths) => other_paths == self_paths,
                _ => false,
            },
            Self::Error(other_paths) => match self {
                Self::Error(self_paths) => other_paths == self_paths,
                _ => false,
            },
        }
    }
}

impl ConflictStatistic {
    pub fn display_as_path(&self) -> String {
        match self {
            ConflictStatistic::Success(success) => {
                let s = success
                    .paths
                    .iter()
                    .enumerate()
                    .map(|(index, p)| {
                        if index == 0 {
                            p.to_string().blue().to_string()
                        } else {
                            p.to_string().green().to_string()
                        }
                    })
                    .join(" <- ");
                format!("{} {}", s, "OK".green())
            }
            ConflictStatistic::Conflict(conflict) => {
                let s = conflict
                    .paths
                    .iter()
                    .enumerate()
                    .map(|(index, p)| {
                        if index == 0 {
                            p.to_string().blue().to_string()
                        } else {
                            if !conflict.tested.contains(&index) {
                                p.to_string().strikethrough().to_string()
                            } else if conflict.failed_at.contains(&index) {
                                p.to_string().red().to_string()
                            } else {
                                p.to_string().green().to_string()
                            }
                        }
                    })
                    .join(" <- ");
                format!("{} {}", s, "CONFLICT".red())
            }
            ConflictStatistic::Error(failure) => {
                let s = failure
                    .paths
                    .iter()
                    .map(|p| p.to_string())
                    .join(" <- ")
                    .strikethrough()
                    .red();
                format!("{} {}:\n{}", s, "ERROR".red(), failure.error.to_string().red())
            }
        }
    }
    // pub fn display_as_list(&self) -> impl Iterator<Item=String> {}
}

pub struct ConflictStatistics {
    ok: Vec<ConflictStatistic>,
    conflict: Vec<ConflictStatistic>,
    error: Vec<ConflictStatistic>,
}

impl ConflictStatistics {
    pub fn new() -> Self {
        Self {
            ok: vec![],
            conflict: vec![],
            error: vec![],
        }
    }
    pub fn from_iter<T: Iterator<Item = ConflictStatistic>>(statistics: T) -> Self {
        let mut new = Self::new();
        for statistic in statistics {
            new.push(statistic);
        }
        new
    }
    pub fn push(&mut self, statistic: ConflictStatistic) {
        match statistic {
            ConflictStatistic::Success(_) => self.ok.push(statistic),
            ConflictStatistic::Conflict(_) => self.conflict.push(statistic),
            ConflictStatistic::Error(_) => self.error.push(statistic),
        }
    }
    pub fn iter_all(&self) -> impl Iterator<Item = &ConflictStatistic> {
        self.iter_ok()
            .chain(self.iter_conflicts())
            .chain(self.iter_errors())
    }
    pub fn iter_ok(&self) -> impl Iterator<Item = &ConflictStatistic> {
        self.ok.iter()
    }
    pub fn iter_conflicts(&self) -> impl Iterator<Item = &ConflictStatistic> {
        self.conflict.iter()
    }
    pub fn iter_errors(&self) -> impl Iterator<Item = &ConflictStatistic> {
        self.error.iter()
    }
    pub fn n_ok(&self) -> usize {
        self.ok.len()
    }
    pub fn n_conflict(&self) -> usize {
        self.conflict.len()
    }
    pub fn n_errors(&self) -> usize {
        self.error.len()
    }
    pub fn contains(&self, statistic: &ConflictStatistic) -> bool {
        self.ok.contains(statistic)
            || self.conflict.contains(statistic)
            || self.error.contains(statistic)
    }
}

impl FromIterator<ConflictStatistic> for ConflictStatistics {
    fn from_iter<T: IntoIterator<Item = ConflictStatistic>>(iter: T) -> Self {
        Self::from_iter(iter.into_iter())
    }
}
#[derive(Debug, Clone)]
pub struct ConflictChecker<'a> {
    interface: &'a GitInterface,
}

impl<'a> ConflictChecker<'a> {
    pub fn new(interface: &'a GitInterface) -> Self {
        Self { interface }
    }

    pub fn check_k_permutations(
        &self,
        paths: Vec<QualifiedPath>,
        k: usize,
    ) -> Result<impl Iterator<Item = ConflictStatistic>, GitError> {
        let iterator = paths.into_iter().permutations(k).map(|perm| {
            let statistic = self.check_chain(&perm);
            self.build_statistic(perm, statistic)
        });
        Ok(iterator)
    }

    pub fn check_permutations_against_base(
        &self,
        targets: Vec<QualifiedPath>,
        base: &QualifiedPath,
        k: usize,
    ) -> Result<impl Iterator<Item = ConflictStatistic>, GitError> {
        let iterator = targets.into_iter().permutations(k).map(|target| {
            let mut to_check: Vec<QualifiedPath> = vec![];
            to_check.push(base.clone());
            to_check.extend(target);
            let statistic = self.check_chain(&to_check);
            self.build_statistic(to_check, statistic)
        });
        Ok(iterator)
    }

    pub fn check_permutations_against_multiple(
        &self,
        left: &Vec<QualifiedPath>,
        right: &Vec<QualifiedPath>,
        k: usize,
    ) -> Result<impl Iterator<Item = ConflictStatistic>, GitError> {
        if k < 1 {
            panic!("k must be at least 1")
        }
        let iterator = left
            .into_iter()
            .flat_map(move |l| {
                right
                    .clone()
                    .into_iter()
                    .permutations(k)
                    .filter_map(move |r| {
                        if r.contains(&l) {
                            None
                        } else {
                            let mut to_check: Vec<QualifiedPath> = vec![l.clone()];
                            to_check.extend(r.iter().map(|p| p.clone()));
                            Some(self.check_k_permutations(to_check, k + 1))
                        }
                    })
                    .flatten()
            })
            .flatten();
        Ok(iterator)
    }

    pub fn clean_up(&mut self) {}

    fn check_chain(
        &self,
        chain: &Vec<QualifiedPath>,
    ) -> Result<(Option<usize>, Vec<usize>), GitError> {
        if chain.len() < 2 {
            panic!("Chain has to contain at least 2 paths")
        }
        let mut failed_at: Option<usize> = None;
        let mut tested: Vec<usize> = vec![];
        let current_path = self.interface.get_current_qualified_path()?;
        let base = &chain[0];
        self.interface.checkout(base)?;
        let temporary = QualifiedPath::from("tmp");
        self.interface.create_branch_no_mut(&temporary)?;
        self.interface.checkout_raw(&temporary)?;
        for (index, path) in chain[1..].iter().enumerate() {
            let success = self.interface.merge(&vec![path.clone()])?.status.success();
            if !success {
                self.interface.abort_merge()?;
                failed_at = Some(index + 1);
                tested.push(index + 1);
                break;
            }
        }
        self.interface.checkout(&current_path)?;
        self.interface.delete_branch(&temporary)?;
        Ok((failed_at, tested))
    }

    fn build_statistic(
        &self,
        paths: Vec<QualifiedPath>,
        result: Result<(Option<usize>, Vec<usize>), GitError>,
    ) -> ConflictStatistic {
        match result {
            Ok((failed_at, tested)) => match failed_at {
                None => ConflictStatistic::Success(MergeSuccess { paths }),
                Some(value) => ConflictStatistic::Conflict(MergeConflict {
                    paths,
                    failed_at: vec![value],
                    tested,
                }),
            },
            Err(e) => ConflictStatistic::Error(MergeError { paths, error: e }),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Conflict2DMatrix {
    matrix: HashMap<QualifiedPath, HashMap<QualifiedPath, i32>>,
    all_keys: Vec<QualifiedPath>,
}

impl Conflict2DMatrix {
    pub fn initialize(paths: &Vec<QualifiedPath>) -> Self {
        let mut matrix: HashMap<QualifiedPath, HashMap<QualifiedPath, i32>> = HashMap::new();
        for combinations in paths.iter().combinations(2) {
            let l = combinations[0];
            let r = combinations[1];

            if matrix.contains_key(&l) {
                matrix.get_mut(l).unwrap().insert(r.clone(), 0);
            } else {
                let mut map: HashMap<QualifiedPath, i32> = HashMap::new();
                map.insert(r.clone(), 0);
                matrix.insert(l.clone(), map);
            }

            if matrix.contains_key(&r) {
                matrix.get_mut(r).unwrap().insert(l.clone(), 0);
            } else {
                let mut map: HashMap<QualifiedPath, i32> = HashMap::new();
                map.insert(l.clone(), 0);
                matrix.insert(r.clone(), map);
            }
        }
        Self {
            matrix,
            all_keys: paths.clone(),
        }
    }

    pub fn insert(&mut self, statistic: ConflictStatistic) {
        match statistic {
            ConflictStatistic::Conflict(conflict) => {
                self.matrix
                    .get_mut(&conflict.paths[0])
                    .unwrap()
                    .insert(conflict.paths[1].clone(), -1);
            }
            _ => {}
        }
    }

    pub fn calculate_best_path_greedy(&self) -> ConflictStatistic {
        let mut has_conflicts = false;
        let mut missing = self.all_keys.clone();
        let start = missing.remove(0);
        let mut final_path = vec![(start, 0)];
        while missing.len() > 0 {
            let mut votes: HashMap<i32, Vec<QualifiedPath>> = HashMap::new();
            for candidate in missing.iter() {
                let mut vote = 0;
                for (p, _) in final_path.iter() {
                    vote += self.matrix[p].get(candidate).unwrap();
                }
                if votes.contains_key(&vote) {
                    votes.get_mut(&vote).unwrap().push(candidate.clone());
                } else {
                    votes.insert(vote, vec![candidate.clone()]);
                }
            }
            let max_vote = votes.keys().max().unwrap();
            if max_vote < &0 {
                has_conflicts = true;
            }
            let max_candidates = &votes[&max_vote];
            let winner = match max_candidates.len() {
                0 => {
                    panic!("Empty candidates should not be possible")
                }
                1 => max_candidates[0].clone(),
                _ => {
                    let start = max_candidates[0].clone();
                    let compatibility = self.calculate_forward_compatibility(&start, &missing);
                    let mut highest_compatible = (start, compatibility);
                    for candidate in max_candidates[1..].iter() {
                        let compatibility =
                            self.calculate_forward_compatibility(&candidate, &missing);
                        if compatibility > highest_compatible.1 {
                            highest_compatible = (candidate.clone(), compatibility);
                        }
                    }
                    highest_compatible.0
                }
            };
            let index: usize = missing
                .iter()
                .enumerate()
                .find_map(|(index, e)| if e == &winner { Some(index) } else { None })
                .unwrap();
            missing.remove(index);
            final_path.push((winner, max_vote.clone()));
        }
        match has_conflicts {
            false => ConflictStatistic::Success(MergeSuccess {
                paths: final_path.into_iter().map(|(path, _)| path).collect(),
            }),
            true => {
                let mut paths: Vec<QualifiedPath> = Vec::new();
                let mut tested: Vec<usize> = Vec::new();
                let mut failed_at: Vec<usize> = Vec::new();
                for (index, (p, i)) in final_path.into_iter().enumerate() {
                    paths.push(p);
                    if i < 0 {
                        failed_at.push(index);
                    }
                    tested.push(index);
                }
                ConflictStatistic::Conflict(MergeConflict {
                    paths,
                    failed_at,
                    tested,
                })
            }
        }
    }

    fn calculate_forward_compatibility(
        &self,
        element: &QualifiedPath,
        missing: &Vec<QualifiedPath>,
    ) -> i32 {
        let table = &self.matrix[element];
        table
            .iter()
            .filter_map(|(k, v)| if missing.contains(k) { Some(*v) } else { None })
            .sum::<i32>()
    }
}

pub struct ConflictAnalyzer<'a> {
    checker: ConflictChecker<'a>,
    context: &'a CommandContext<'a>,
}

impl<'a> ConflictAnalyzer<'a> {
    pub fn new(checker: ConflictChecker<'a>, context: &'a CommandContext<'a>) -> Self {
        Self { checker, context }
    }

    pub fn calculate_2d_heuristics_matrix_with_merge_base(
        &mut self,
        paths: &Vec<QualifiedPath>,
        base: &QualifiedPath,
    ) -> Result<Conflict2DMatrix, GitError> {
        let mut all = vec![base.clone()];
        all.extend(paths.iter().map(|path| path.clone()));
        let mut matrix = Conflict2DMatrix::initialize(&all);

        let mut conflicting_with_base: Vec<QualifiedPath> = vec![];
        self.context.debug("Checking against base pairwise");
        for s in self
            .checker
            .check_permutations_against_base(paths.clone(), &base, 1)?
        {
            self.context.debug(s.display_as_path());
            match s {
                ConflictStatistic::Conflict(merge) => {
                    conflicting_with_base.push(merge.paths[1].clone());
                }
                ConflictStatistic::Error(merge) => return Err(merge.error),
                _ => {}
            }
        }
        let to_test_with_base: Vec<QualifiedPath> = paths
            .iter()
            .filter(|path| !conflicting_with_base.contains(path))
            .cloned()
            .collect();

        self.context.debug("Checking successful against base");
        for with_base in
            self.checker
                .check_permutations_against_base(to_test_with_base, &base, 2)?
        {
            self.context.debug(with_base.display_as_path());
            let altered: ConflictStatistic = match with_base {
                ConflictStatistic::Success(success) => ConflictStatistic::Success(MergeSuccess {
                    paths: success.paths[1..].to_vec(),
                }),
                ConflictStatistic::Conflict(conflict) => {
                    ConflictStatistic::Conflict(MergeConflict {
                        paths: conflict.paths[1..].to_vec(),
                        failed_at: conflict.failed_at.iter().map(|i| i - 1).collect(),
                        tested: conflict.tested.iter().map(|i| i - 1).collect(),
                    })
                }
                ConflictStatistic::Error(error) => return Err(error.error),
            };
            matrix.insert(altered);
        }
        self.context.debug("Checking conflicting without base");
        for without_base in
            self.checker
                .check_permutations_against_multiple(&conflicting_with_base, &paths, 1)?
        {
            self.context.debug(without_base.display_as_path());
            matrix.insert(without_base);
        }
        self.checker.clean_up();
        Ok(matrix)
    }
}

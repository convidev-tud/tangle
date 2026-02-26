use crate::git::error::GitError;
use crate::git::interface::GitInterface;
use crate::model::QualifiedPath;
use colored::Colorize;
use std::fmt::Display;

#[derive(Debug)]
pub enum ConflictStatistic {
    OK((QualifiedPath, QualifiedPath)),
    CONFLICT((QualifiedPath, QualifiedPath)),
    ERROR((QualifiedPath, QualifiedPath), GitError),
}

impl PartialEq for ConflictStatistic {
    fn eq(&self, other: &Self) -> bool {
        match other {
            Self::OK((other_l, other_r)) => match self {
                Self::OK((self_l, self_r)) => self_l == other_l && self_r == other_r,
                _ => false,
            },
            Self::CONFLICT((other_l, other_r)) => match self {
                Self::CONFLICT((self_l, self_r)) => self_l == other_l && self_r == other_r,
                _ => false,
            },
            Self::ERROR((other_l, other_r), _) => match self {
                Self::ERROR((self_l, self_r), _) => self_l == other_l && self_r == other_r,
                _ => false,
            },
        }
    }
}

impl Display for ConflictStatistic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let formatted = match self {
            ConflictStatistic::OK((l, r)) => {
                format!("{} and {} ", l, r) + "OK".green().to_string().as_str()
            }
            ConflictStatistic::CONFLICT((l, r)) => {
                format!("{} and {} ", l, r) + "CONFLICT".red().to_string().as_str()
            }
            ConflictStatistic::ERROR((l, r), _) => {
                format!("{} and {} ", l, r) + "ERROR".green().to_string().as_str()
            }
        };
        f.write_str(formatted.as_str())
    }
}
impl Into<String> for ConflictStatistic {
    fn into(self) -> String {
        self.to_string()
    }
}
impl Into<String> for &ConflictStatistic {
    fn into(self) -> String {
        self.to_string()
    }
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
            ConflictStatistic::OK(_) => self.ok.push(statistic),
            ConflictStatistic::CONFLICT(_) => self.conflict.push(statistic),
            ConflictStatistic::ERROR(_, _) => self.error.push(statistic),
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

pub enum ConflictCheckBaseBranch {
    Current,
    Custom(QualifiedPath),
}

pub struct ConflictChecker<'a> {
    interface: &'a GitInterface,
    base_branch: ConflictCheckBaseBranch,
}

impl<'a> ConflictChecker<'a> {
    pub fn new(interface: &'a GitInterface, base_branch: ConflictCheckBaseBranch) -> Self {
        Self { interface, base_branch }
    }

    pub fn check_all(
        &self,
        paths: &Vec<QualifiedPath>,
    ) -> Result<impl Iterator<Item = ConflictStatistic>, GitError> {
        let mut feature_combinations: Vec<(&QualifiedPath, &QualifiedPath)> = Vec::new();
        for (i, path) in paths.iter().enumerate() {
            for part in paths[i + 1..].iter() {
                feature_combinations.push((path, part));
            }
        }

        let iterator = feature_combinations.into_iter().map(|(l, r)| {
            let statistic = self.check_two(l, r);
            self.build_statistic(l.clone(), r.clone(), statistic)
        });
        Ok(iterator)
    }

    pub fn check_1_to_n(
        &self,
        source: &QualifiedPath,
        targets: &Vec<QualifiedPath>,
    ) -> Result<impl Iterator<Item = ConflictStatistic>, GitError> {
        let iterator = targets.into_iter().map(move |target| {
            let statistic = self.check_two(source, target);
            self.build_statistic(source.clone(), target.clone(), statistic)
        });
        Ok(iterator)
    }

    fn check_two(&self, l: &QualifiedPath, r: &QualifiedPath) -> Result<bool, GitError> {
        let current_path = self.interface.get_current_qualified_path()?;
        match &self.base_branch {
            ConflictCheckBaseBranch::Custom(path) => {
                self.interface.checkout(path)?;
            }
            _ => {}
        };
        let temporary = QualifiedPath::from("tmp");
        self.interface.create_branch_no_mut(&temporary)?;
        self.interface.checkout_raw(&temporary)?;
        let success = self
            .interface
            .merge(&vec![l.clone(), r.clone()])?
            .status
            .success();
        if !success {
            self.interface.abort_merge()?;
        }
        self.interface.checkout(&current_path)?;
        self.interface.delete_branch(&temporary)?;
        Ok(success)
    }

    fn build_statistic(
        &self,
        l: QualifiedPath,
        r: QualifiedPath,
        result: Result<bool, GitError>,
    ) -> ConflictStatistic {
        match result {
            Ok(stat) => match stat {
                true => ConflictStatistic::OK((l, r)),
                false => ConflictStatistic::CONFLICT((l, r)),
            },
            Err(e) => ConflictStatistic::ERROR((l, r), e),
        }
    }
}

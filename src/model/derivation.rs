use crate::model::QualifiedPath;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureMetadata {
    path: String,
}
impl FeatureMetadata {
    pub fn new<S: Into<String>>(path: S) -> Self {
        Self { path: path.into() }
    }
    pub fn from_qualified_paths(paths: &Vec<QualifiedPath>) -> Vec<Self> {
        paths.iter().map(|path| Self::new(path.clone())).collect()
    }
    pub fn qualified_paths(metadata: &Vec<Self>) -> Vec<QualifiedPath> {
        metadata.iter().map(|m| m.get_qualified_path()).collect()
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
    pub fn from_previously_finished<S: Into<String>>(
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
    pub fn from_json<S: Into<String>>(content: S) -> serde_json::error::Result<Self> {
        serde_json::from_str::<Self>(&content.into())
    }
    pub fn to_json(&self) -> serde_json::error::Result<String> {
        serde_json::to_string(&self)
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
    pub fn reorder_missing(&mut self, new_order: &Vec<QualifiedPath>) {
        let old_missing = FeatureMetadata::qualified_paths(&self.missing);
        let mut new_missing: Vec<QualifiedPath> = Vec::new();
        for new in new_order.iter() {
            if !old_missing.contains(new) {
                panic!("Cannot reorder: tried to introduce new feature")
            }
            new_missing.push(new.clone());
        }
        self.missing = FeatureMetadata::from_qualified_paths(&new_missing);
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
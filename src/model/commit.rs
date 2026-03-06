use serde::Deserialize;
use crate::model::DerivationMetadata;

pub trait Commit {
    fn get_hash(&self) -> &String;
    fn get_message(&self) -> &String;
}

#[derive(Debug, Clone, Deserialize)]
pub struct BaseCommit {
    hash: String,
    message: String,
}

impl PartialEq for BaseCommit {
    fn eq(&self, other: &Self) -> bool {
        other.hash == self.hash
    }

    fn ne(&self, other: &Self) -> bool {
        other.hash != self.hash
    }
}

impl Commit for BaseCommit {
    fn get_hash(&self) -> &String {
        &self.hash
    }
    fn get_message(&self) -> &String {
        &self.message
    }
}

impl BaseCommit {
    pub fn new<S1: Into<String>, S2: Into<String>>(hash: S1, message: S2) -> Self {
        Self {
            hash: hash.into(),
            message: message.into(),
        }
    }
}

const DERIVATION_COMMENT: &str = "# DO NOT EDIT OR REMOVE THIS COMMIT\nDERIVATION STATUS\n";

struct DerivationCommit {
    base_commit: BaseCommit,
    metadata: DerivationMetadata,
}

impl Commit for DerivationCommit {
    fn get_hash(&self) -> &String {
        self.base_commit.get_hash()
    }

    fn get_message(&self) -> &String {
        self.base_commit.get_message()
    }
}

impl DerivationCommit {
    pub fn from_base_commit(base_commit: BaseCommit) -> Option<serde_json::error::Result<Self>> {
        if !base_commit.get_message().contains(DERIVATION_COMMENT) {
            return None;
        }
        let metadata = DerivationMetadata::from_json(base_commit.get_message());
        match metadata {
            Ok(metadata) => Some(Ok(Self { base_commit, metadata })),
            Err(e) => Some(Err(e)),
        }
    }
    pub fn make_derivation_message(metadata: DerivationMetadata) -> serde_json::error::Result<String> {
        let base = DERIVATION_COMMENT.to_string();
        let serialized = metadata.to_json()?;
        Ok(base + serialized.as_str())
    }
    pub fn get_metadata(&self) -> &DerivationMetadata {
        &self.metadata
    }
}

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Commit {
    hash: String,
    message: String,
}

impl PartialEq for Commit {
    fn eq(&self, other: &Self) -> bool {
        other.hash == self.hash
    }

    fn ne(&self, other: &Self) -> bool {
        other.hash != self.hash
    }
}

impl Commit {
    pub fn new<S1: Into<String>, S2: Into<String>>(hash: S1, message: S2) -> Self {
        Self {
            hash: hash.into(),
            message: message.into(),
        }
    }
    pub fn get_hash(&self) -> &String {
        &self.hash
    }
    pub fn get_message(&self) -> &String {
        &self.message
    }
}

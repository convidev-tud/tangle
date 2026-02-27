use colored::Colorize;
use std::fmt::{Display, Formatter};
use std::ops::{Add, Index};

const SEPARATOR: char = '/';

#[derive(Clone, Debug, Hash, Eq, Ord, PartialOrd)]
pub struct QualifiedPath {
    path: Vec<String>,
}
impl From<String> for QualifiedPath {
    fn from(value: String) -> Self {
        let mut qualified_path = Self::new();
        qualified_path.push(value);
        qualified_path
    }
}
impl From<&String> for QualifiedPath {
    fn from(value: &String) -> Self {
        Self::from(value.to_string())
    }
}
impl From<&str> for QualifiedPath {
    fn from(value: &str) -> Self {
        Self::from(value.to_string())
    }
}
impl From<Vec<String>> for QualifiedPath {
    fn from(value: Vec<String>) -> Self {
        let mut path = Self::new();
        for v in value {
            path.push(v);
        }
        path
    }
}
impl From<QualifiedPath> for String {
    fn from(value: QualifiedPath) -> Self {
        value.to_string()
    }
}
impl PartialEq for QualifiedPath {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }

    fn ne(&self, other: &Self) -> bool {
        self.path != other.path
    }
}
impl PartialEq<String> for QualifiedPath {
    fn eq(&self, other: &String) -> bool {
        self.to_string() == *other
    }

    fn ne(&self, other: &String) -> bool {
        self.to_string() != *other
    }
}
impl PartialEq<&str> for QualifiedPath {
    fn eq(&self, other: &&str) -> bool {
        self.to_string() == *other
    }

    fn ne(&self, other: &&str) -> bool {
        self.to_string() != *other
    }
}
impl Add for QualifiedPath {
    type Output = QualifiedPath;

    fn add(self, rhs: Self) -> Self::Output {
        let mut next_index = self.len();
        let mut new_path = self;
        if new_path.last_is(&QualifiedPath::from("")) && new_path.len() > 1 {
            new_path = new_path.strip_n_right(new_path.len() - 1);
        }
        for (i, part) in rhs.iter_string().enumerate() {
            match part.as_str() {
                "." => {}
                ".." => {
                    if next_index > 0 {
                        next_index -= 1;
                        new_path = new_path.strip_n_right(next_index);
                    }
                }
                "" => {
                    if i == 0 && rhs.len() > 1 {
                        return rhs.clone();
                    } else if i == rhs.len() - 1 || new_path.is_empty() {
                        new_path.push(part.to_string())
                    }
                }
                _ => {
                    new_path.push(part.to_string());
                    next_index += 1;
                }
            }
        }
        new_path
    }
}
impl Display for QualifiedPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.path.join("/").as_str())
    }
}
impl Index<usize> for QualifiedPath {
    type Output = String;

    fn index(&self, index: usize) -> &Self::Output {
        &self.path[index]
    }
}
impl QualifiedPath {
    pub fn new() -> Self {
        Self { path: Vec::new() }
    }
    pub fn to_git_branch(&self) -> String {
        let trimmed_path = self.trim_whitespaces();
        let path = trimmed_path.path;
        match path.len() {
            1 => path[0].to_string(),
            _ => {
                let mut prefix = path[..path.len() - 1]
                    .iter()
                    .map(|x| "_".to_string() + x)
                    .collect::<Vec<_>>();
                prefix.push(path[path.len() - 1].to_string());
                prefix.join("/")
            }
        }
    }
    pub fn push<S: Into<String>>(&mut self, path: S) {
        let qualified_str = path.into().replace("*", "").replace("_", "");
        for split in qualified_str.trim().split(SEPARATOR) {
            self.path.push(split.to_string());
        }
    }
    pub fn strip_n(&self, n_left: usize, n_right: usize) -> QualifiedPath {
        QualifiedPath::from(self.path[n_left..n_right].to_vec())
    }
    pub fn strip_n_left(&self, n: usize) -> QualifiedPath {
        self.strip_n(n, self.path.len())
    }
    pub fn strip_n_right(&self, n: usize) -> QualifiedPath {
        self.strip_n(0, n)
    }
    pub fn trim_whitespaces(&self) -> QualifiedPath {
        let mut new_path = self.path.clone();
        match new_path.first() {
            Some(value) => {
                if value == "" {
                    new_path.remove(0);
                }
            }
            None => {}
        }
        match new_path.last() {
            Some(value) => {
                if value == "" {
                    new_path.remove(new_path.len() - 1);
                }
            }
            None => {}
        }
        QualifiedPath::from(new_path)
    }
    pub fn replace<S: Into<String>>(&self, index: usize, value: S) -> QualifiedPath {
        let mut new_path = self.path.clone();
        new_path.insert(index, value.into());
        QualifiedPath::from(new_path)
    }
    pub fn first(&self) -> Option<QualifiedPath> {
        Some(QualifiedPath::from(self.path.first()?.clone()))
    }
    pub fn last(&self) -> Option<&String> {
        self.path.last()
    }
    pub fn is_empty(&self) -> bool {
        self.path.is_empty()
    }
    pub fn iter(&self) -> impl Iterator<Item = QualifiedPath> {
        self.iter_string().map(|s| QualifiedPath::from(s.clone()))
    }
    pub fn iter_string(&self) -> impl Iterator<Item = &String> {
        self.path.iter()
    }
    pub fn get(&self, index: usize) -> Option<QualifiedPath> {
        Some(QualifiedPath::from(self.path.get(index)?.clone()))
    }
    pub fn starts_with(&self, prefix: &QualifiedPath) -> bool {
        self.to_string().starts_with(&prefix.to_string())
    }
    pub fn last_is(&self, suffix: &QualifiedPath) -> bool {
        self.last() == suffix.last()
    }
    pub fn len(&self) -> usize {
        self.path.len()
    }
    pub fn as_dir(&self) -> QualifiedPath {
        let mut new_path = self.path.clone();
        new_path.push("".to_string());
        QualifiedPath::from(new_path)
    }
    pub fn as_absolute(&self) -> QualifiedPath {
        let mut new_path = self.path.clone();
        new_path.insert(0, "".to_string());
        QualifiedPath::from(new_path)
    }
    pub fn is_dir(&self) -> bool {
        self.path.len() > 1 && self.last().unwrap() == ""
    }
    pub fn is_absolute(&self) -> bool {
        self.path.len() > 0 && self.first().unwrap() == ""
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qualified_path_from_qualified() {
        assert_eq!(QualifiedPath::from("foo/bar").path, vec!["foo", "bar"]);
        assert_eq!(QualifiedPath::from("/foo/bar").path, vec!["", "foo", "bar"]);
        assert_eq!(QualifiedPath::from("/foo/bar").to_string(), "/foo/bar");
        assert_eq!(QualifiedPath::from("foo/").path, vec!["foo", ""]);
        assert_eq!(QualifiedPath::from("/").path, vec!["", ""]);
    }

    #[test]
    fn test_qualified_path_from_git_branch() {
        assert_eq!(QualifiedPath::from("_foo/bar").path, vec!["foo", "bar"]);
        assert_eq!(
            QualifiedPath::from("_foo/bar".to_string()).path,
            vec!["foo", "bar"]
        );
        assert_eq!(
            QualifiedPath::from("_foo/_bar/baz").path,
            vec!["foo", "bar", "baz"]
        );
    }

    #[test]
    fn test_qualified_path_to_git_branch() {
        assert_eq!(QualifiedPath::from("foo/bar").to_git_branch(), "_foo/bar");
        assert_eq!(QualifiedPath::from("/foo/bar").to_git_branch(), "_foo/bar");
    }

    #[test]
    fn test_qualified_path_add_empty() {
        let l = QualifiedPath::new();
        let r = QualifiedPath::from("foo/bar");
        assert_eq!(l + r, QualifiedPath::from("foo/bar"));

        let l = QualifiedPath::new();
        let r = QualifiedPath::from("/foo/bar");
        assert_eq!(l + r, QualifiedPath::from("/foo/bar"));
    }

    #[test]
    fn test_qualified_path_add_absolute() {
        let l = QualifiedPath::from("foo");
        let r = QualifiedPath::from("bar/baz");
        assert_eq!(l + r, QualifiedPath::from("foo/bar/baz"));

        let l = QualifiedPath::from("");
        let r = QualifiedPath::from("bar/baz");
        assert_eq!((l + r).path, vec!["", "bar", "baz"]);

        let l = QualifiedPath::from("foo/");
        let r = QualifiedPath::from("bar/baz");
        assert_eq!(l + r, QualifiedPath::from("foo/bar/baz"));
    }

    #[test]
    fn test_qualified_path_add_relative() {
        let l = QualifiedPath::from("foo");
        let r = QualifiedPath::from("..");
        assert_eq!(l + r, QualifiedPath::new());

        let l = QualifiedPath::from("foo");
        let r = QualifiedPath::from("./bar");
        assert_eq!(l + r, QualifiedPath::from("foo/bar"));

        let l = QualifiedPath::from("foo");
        let r = QualifiedPath::from("./");
        assert_eq!(l + r, QualifiedPath::from("foo/"));

        let l = QualifiedPath::from("foo");
        let r = QualifiedPath::from("../bar");
        assert_eq!(l + r, QualifiedPath::from("bar"));

        let l = QualifiedPath::from("foo/bar");
        let r = QualifiedPath::from("../baz");
        assert_eq!(l + r, QualifiedPath::from("foo/baz"));

        let l = QualifiedPath::from("foo/bar");
        let r = QualifiedPath::from("../../baz");
        assert_eq!(l + r, QualifiedPath::from("baz"));

        let l = QualifiedPath::from("foo/bar");
        let r = QualifiedPath::from("../../../../../../baz");
        assert_eq!(l + r, QualifiedPath::from("baz"));

        let l = QualifiedPath::from("foo/bar");
        let r = QualifiedPath::from("baz/../baz/../baz/../baz");
        assert_eq!(l + r, QualifiedPath::from("foo/bar/baz"));

        let l = QualifiedPath::from("foo/bar");
        let r = QualifiedPath::from("../baz/../baz/../baz");
        assert_eq!(l + r, QualifiedPath::from("foo/baz"));
    }

    #[test]
    fn test_qualified_path_add_whitespaces() {
        let l = QualifiedPath::from("foo");
        let r = QualifiedPath::from("");
        assert_eq!(l + r, QualifiedPath::from("foo/"));

        let l = QualifiedPath::from("foo");
        let r = QualifiedPath::from("/bar/baz");
        assert_eq!(l + r, QualifiedPath::from("/bar/baz"));
    }

    #[test]
    fn test_qualified_path_trim() {
        let path = QualifiedPath::from("foo/bar");
        assert_eq!(path.strip_n(0, path.len() - 1).path, vec!["foo"]);
    }

    #[test]
    fn test_qualified_path_as_absolute() {
        let path = QualifiedPath::from("foo/bar");
        let absolute = path.as_absolute();
        assert!(absolute.is_absolute());
        assert_eq!(absolute, "/foo/bar");
    }
}

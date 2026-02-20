use crate::model::QualifiedPath;
use std::collections::HashSet;

pub struct RelativePathCompleter {
    reference_path: QualifiedPath,
}
impl RelativePathCompleter {
    pub fn new(reference_path: QualifiedPath) -> Self {
        if reference_path.is_empty() {
            panic!("Reference path must not be empty")
        }
        Self { reference_path }
    }
    pub fn complete(
        &self,
        prefix: QualifiedPath,
        paths: impl Iterator<Item = QualifiedPath>,
    ) -> Vec<String> {
        let filtered: Vec<QualifiedPath> = self
            .transform_and_filter_path(prefix.clone(), paths)
            .collect();
        match filtered.len() {
            0 => vec![],
            1 => vec![filtered[0].to_string()],
            _ => {
                let current_index = prefix.len();
                let all = filtered
                    .iter()
                    .map(|path| {
                        let to_index = path.strip_n_right(current_index);
                        let to_return = if path.len() == current_index {
                            to_index
                        } else {
                            to_index.as_dir()
                        };
                        to_return.to_string()
                    })
                    .collect::<HashSet<_>>()
                    .into_iter()
                    .collect::<Vec<String>>();
                if all.len() == 1 {
                    filtered.iter().map(|path| path.to_string()).collect()
                } else {
                    all
                }
            }
        }
    }
    fn transform_and_filter_path<'a>(
        &self,
        prefix: QualifiedPath,
        paths: impl Iterator<Item = QualifiedPath>,
    ) -> impl Iterator<Item = QualifiedPath> {
        let transformed_prefix = if prefix.last().is_some() {
            match prefix.last().unwrap().as_str() {
                "." | ".." => prefix.as_dir(),
                _ => prefix,
            }
        } else {
            prefix
        };
        let current_position = self.reference_path.clone() + transformed_prefix.clone();
        let current_index = current_position.len() - 1;
        paths.filter_map(move |path| {
            if !path.starts_with(&current_position) {
                return None;
            }
            if path.len() <= current_index {
                return None;
            }
            let new_path = transformed_prefix.strip_n_right(transformed_prefix.len() - 1)
                + path.strip_n_left(current_index);
            Some(new_path)
        })
    }
}

#[cfg(test)]
pub mod test_utils {
    use crate::model::QualifiedPath;

    pub fn setup_qualified_paths() -> Vec<QualifiedPath> {
        vec![
            QualifiedPath::from("/foo"),
            QualifiedPath::from("/foo/bar/baz1"),
            QualifiedPath::from("/foo/bar/baz2"),
            QualifiedPath::from("/foo/abc/def"),
            QualifiedPath::from("/foo/abc"),
            QualifiedPath::from("/bar"),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::completion::path::test_utils::setup_qualified_paths;

    #[test]
    fn test_relative_path_completion_from_virtual_root() {
        let paths = setup_qualified_paths();
        let completion = RelativePathCompleter::new(QualifiedPath::from(""));

        let mut direct = completion.complete(QualifiedPath::from(""), paths.clone().into_iter());
        direct.sort();
        assert_eq!(direct, vec!["bar", "foo", "foo/",]);

        let mut prefixed1 =
            completion.complete(QualifiedPath::from("/f"), paths.clone().into_iter());
        prefixed1.sort();
        assert_eq!(prefixed1, vec!["/foo", "/foo/"]);

        let mut prefixed2 =
            completion.complete(QualifiedPath::from("/"), paths.clone().into_iter());
        prefixed2.sort();
        assert_eq!(prefixed2, vec!["/bar", "/foo", "/foo/"]);
    }

    #[test]
    fn test_relative_path_completion_relative_identifier_current_path() {
        let paths = setup_qualified_paths();
        let completion = RelativePathCompleter::new(QualifiedPath::from("/foo"));

        let mut direct = completion.complete(QualifiedPath::from("."), paths.clone().into_iter());
        direct.sort();
        assert_eq!(
            direct,
            vec!["./abc", "./abc/def", "./bar/baz1", "./bar/baz2"]
        );

        let mut direct_with_slash =
            completion.complete(QualifiedPath::from("./"), paths.clone().into_iter());
        direct_with_slash.sort();
        assert_eq!(direct_with_slash, vec!["./abc", "./abc/", "./bar/"]);

        let mut prefixed =
            completion.complete(QualifiedPath::from("./a"), paths.clone().into_iter());
        prefixed.sort();
        assert_eq!(prefixed, vec!["./abc", "./abc/"]);

        let mut consecutive = completion.complete(QualifiedPath::from("./b"), paths.into_iter());
        consecutive.sort();
        assert_eq!(consecutive, vec!["./bar/baz1", "./bar/baz2"]);
    }

    #[test]
    fn test_relative_path_completion_relative_identifier_previous_path() {
        let paths = setup_qualified_paths();
        let completion = RelativePathCompleter::new(QualifiedPath::from("/foo"));

        let mut direct = completion.complete(QualifiedPath::from("../"), paths.clone().into_iter());
        direct.sort();
        assert_eq!(direct, vec!["../bar", "../foo", "../foo/"]);

        let mut consecutive =
            completion.complete(QualifiedPath::from("../foo/"), paths.clone().into_iter());
        consecutive.sort();
        assert_eq!(
            consecutive,
            vec!["../foo/abc", "../foo/abc/", "../foo/bar/"]
        );

        let mut previous_of_previous =
            completion.complete(QualifiedPath::from("abc/../../"), paths.into_iter());
        previous_of_previous.sort();
        assert_eq!(
            previous_of_previous,
            vec!["abc/../../bar", "abc/../../foo", "abc/../../foo/"]
        );
    }

    #[test]
    fn test_relative_path_completion_current_path() {
        let paths = setup_qualified_paths();
        let completion = RelativePathCompleter::new(QualifiedPath::from("/foo"));

        let mut direct = completion.complete(QualifiedPath::from(""), paths.clone().into_iter());
        direct.sort();
        assert_eq!(direct, vec!["abc", "abc/", "bar/"]);

        let mut prefixed = completion.complete(QualifiedPath::from("a"), paths.clone().into_iter());
        prefixed.sort();
        assert_eq!(prefixed, vec!["abc", "abc/"]);

        let mut consecutive = completion.complete(QualifiedPath::from("b"), paths.into_iter());
        consecutive.sort();
        assert_eq!(consecutive, vec!["bar/baz1", "bar/baz2"]);
    }

    #[test]
    #[should_panic]
    fn test_relative_path_completion_empty_reference() {
        RelativePathCompleter::new(QualifiedPath::new());
    }
}

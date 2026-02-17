use crate::cli::completion::RelativePathCompleter;
use crate::model::QualifiedPath;
use clap::{Arg, ArgAction, Command};
use std::collections::HashMap;
use std::ops::Range;

#[derive(Debug, Clone)]
pub struct CompletionHelper<'a> {
    command: &'a Command,
    cli_content: Vec<&'a str>,
}
impl<'a> CompletionHelper<'a> {
    pub fn new(command: &'a Command, appendix: Vec<&'a str>) -> Self {
        Self { command, cli_content: appendix }
    }
    pub fn get_last(&self) -> Option<String> {
        Some(self.cli_content.last()?.to_string())
    }

    fn currently_editing_with_range(&self) -> Option<(Range<usize>, &Arg)> {
        let mut current_option: Option<&Arg> = None;
        let mut current_option_start: usize = 0;
        let mut positionals = self.command.get_positionals();
        let mut current_positional: Option<&Arg> = None;
        let mut current_positional_start: usize = 0;
        // check if the last arg is still edited
        fn is_last_option(
            index: usize,
            current_option: Option<&Arg>,
            current_option_start: usize,
        ) -> bool {
            if current_option.is_none() {
                return false;
            }
            match current_option.unwrap().get_action() {
                ArgAction::Set => current_option_start == index - 1,
                ArgAction::Append => current_option_start < index,
                _ => false,
            }
        }
        fn is_last_positional(
            index: usize,
            current_positional: Option<&Arg>,
            current_positional_start: usize,
        ) -> bool {
            if current_positional.is_none() {
                return false;
            }
            match current_positional.unwrap().get_action() {
                ArgAction::Set => current_positional_start == index,
                ArgAction::Append => current_positional_start <= index,
                _ => false,
            }
        }
        // match appendix index to argument
        let cmd_to_index: HashMap<usize, &Arg> = self
            .cli_content
            .iter()
            .enumerate()
            .filter_map(|(index, element)| {
                if element.to_string() == self.command.get_name() {
                    return None;
                }
                // checks if the current one is an option name
                let found_option = self.command.get_opts().find(|o| {
                    let found_short = match o.get_short() {
                        Some(short) => {
                            ("-".to_string() + short.to_string().as_str()) == element.to_string()
                        }
                        None => false,
                    };
                    let found_long = match o.get_long() {
                        Some(long) => ("--".to_string() + long) == element.to_string(),
                        None => false,
                    };
                    found_short || found_long
                });
                let maybe_option: Option<(usize, &Arg)> = match found_option {
                    // if currently an option, save the index
                    Some(option) => {
                        current_option = Some(option);
                        current_option_start = index;
                        return None;
                    }
                    // if not, check if the last option is still edited
                    None => {
                        if is_last_option(index, current_option, current_option_start) {
                            Some((index, current_option.unwrap()))
                        } else {
                            None
                        }
                    }
                };
                if maybe_option.is_some() {
                    return Some(maybe_option.unwrap());
                }
                // if no optional, move on to positionals
                if is_last_positional(index, current_positional, current_positional_start) {
                    return Some((index, current_positional.unwrap()));
                }
                match positionals.next() {
                    Some(positional) => {
                        current_positional_start = index;
                        current_positional = Some(positional);
                        Some((index, positional))
                    }
                    None => None,
                }
            })
            .collect();

        let current_cmd = cmd_to_index.get(&(self.cli_content.len() - 1))?;
        let end: usize = self.cli_content.len() - 1;
        let mut start: usize = end;
        for (i, arg) in cmd_to_index.iter() {
            if arg == current_cmd && i < &start {
                start = *i;
            }
        }
        Some((Range { start, end }, current_cmd))
    }
    /// Returns if the passed target is the currently one edited on the console.
    ///
    /// Examples:
    /// ```bash
    /// mytool foo // foo is edited
    /// mytool foo bar // foo is edited, if curser remains on bar
    /// mytool foo bar abc // foo is not edited
    /// ```
    pub fn currently_editing(&self) -> Option<&Arg> {
        Some(self.currently_editing_with_range()?.1)
    }
    pub fn get_appendix_of_currently_edited(&self) -> Vec<&str> {
        if self.cli_content.len() < 3 {
            return vec![];
        }
        let maybe_currently_editing = self.currently_editing_with_range();
        if maybe_currently_editing.is_none() {
            return self.cli_content[1..self.cli_content.len() - 1].to_vec();
        }
        let currently_editing = maybe_currently_editing.unwrap().0;
        self.cli_content[currently_editing.start..self.cli_content.len() - 1].to_vec()
    }
    pub fn complete_qualified_paths(
        &self,
        reference: QualifiedPath,
        paths: impl Iterator<Item = QualifiedPath>,
        ignore_existing_occurrences: bool,
    ) -> Vec<String> {
        let maybe_last = self.get_last();
        if maybe_last.is_none() {
            return vec![];
        }
        if ignore_existing_occurrences {
            RelativePathCompleter::new(reference.clone()).complete(
                QualifiedPath::from(maybe_last.unwrap()),
                self.treat_existing_occurrences(&reference, paths),
            )
        } else {
            RelativePathCompleter::new(reference)
                .complete(QualifiedPath::from(maybe_last.unwrap()), paths)
        }
    }
    fn treat_existing_occurrences(
        &self,
        reference: &QualifiedPath,
        paths: impl Iterator<Item = QualifiedPath>,
    ) -> impl Iterator<Item = QualifiedPath> {
        let currently_editing_appendix = self.get_appendix_of_currently_edited();
        paths.filter_map(move |path| {
            if currently_editing_appendix
                .contains(&path.strip_n_left(reference.len()).to_string().as_str())
            {
                None
            } else {
                Some(path)
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_command() -> Command {
        Command::new("mytool")
            .arg(Arg::new("option1").long("option1").short('a'))
            .arg(
                Arg::new("option2")
                    .long("option2")
                    .short('b')
                    .action(ArgAction::SetTrue),
            )
            .arg(Arg::new("pos1"))
            .arg(Arg::new("pos2").action(ArgAction::Append))
    }
    fn setup_qualified_paths() -> Vec<QualifiedPath> {
        vec![
            QualifiedPath::from("foo"),
            QualifiedPath::from("foo/bar/baz1"),
            QualifiedPath::from("foo/bar/baz2"),
            QualifiedPath::from("foo/abc/def"),
            QualifiedPath::from("foo/abc"),
        ]
    }

    #[test]
    fn test_currently_editing_empty() {
        let cmd = setup_test_command();
        let appendix = vec!["mytool"];
        let helper = CompletionHelper::new(&cmd, appendix);
        assert_eq!(helper.currently_editing(), None);
    }
    #[test]
    fn test_currently_editing_one_option() {
        let cmd = setup_test_command();
        let appendix = vec!["mytool", "--option1", ""];
        let helper = CompletionHelper::new(&cmd, appendix);
        assert_eq!(
            helper.currently_editing().unwrap().get_id().as_str(),
            "option1"
        );
    }
    #[test]
    fn test_currently_editing_one_option_one_positional() {
        let cmd = setup_test_command();
        let appendix = vec!["mytool", "--option1", "abc", ""];
        let helper = CompletionHelper::new(&cmd, appendix);
        assert_eq!(
            helper.currently_editing().unwrap().get_id().as_str(),
            "pos1"
        );
    }
    #[test]
    fn test_currently_editing_one_positional() {
        let cmd = setup_test_command();
        let appendix = vec!["mytool", "abc"];
        let helper = CompletionHelper::new(&cmd, appendix);
        assert_eq!(
            helper.currently_editing().unwrap().get_id().as_str(),
            "pos1".to_string()
        );
    }
    #[test]
    fn test_currently_editing_append() {
        let cmd = setup_test_command();
        let appendix = vec!["mytool", "abc", "a", "b", "c", "d"];
        let helper = CompletionHelper::new(&cmd, appendix);
        assert_eq!(
            helper.currently_editing().unwrap().get_id().as_str(),
            "pos2"
        );
    }
    #[test]
    fn test_currently_editing_boolean() {
        let cmd = setup_test_command();
        let appendix = vec!["mytool", "-b", ""];
        let helper = CompletionHelper::new(&cmd, appendix);
        assert_eq!(
            helper.currently_editing().unwrap().get_id().as_str(),
            "pos1".to_string()
        );
    }
    #[test]
    fn test_complete_qualified_path_stepwise_ignore_prior_occurrences() {
        let cmd = setup_test_command();
        let appendix = vec!["mytool", "abc", "foo/bar/baz1", "foo/b"];
        let helper = CompletionHelper::new(&cmd, appendix);
        let paths = setup_qualified_paths();
        let mut result =
            helper.complete_qualified_paths(QualifiedPath::new(), paths.into_iter(), true);
        result.sort();
        assert_eq!(result, vec!["foo/bar/baz2",]);
    }
    #[test]
    fn test_complete_qualified_path_stepwise_do_not_ignore_current_occurrences() {
        let cmd = setup_test_command();
        let appendix = vec!["mytool", "abc", "foo/bar/baz1"];
        let helper = CompletionHelper::new(&cmd, appendix);
        let paths = setup_qualified_paths();
        let mut result =
            helper.complete_qualified_paths(QualifiedPath::new(), paths.into_iter(), true);
        result.sort();
        assert_eq!(result, vec!["foo/bar/baz1",]);
    }
    #[test]
    fn test_get_all_of_currently_edited() {
        let cmd = setup_test_command();
        let appendix = vec!["mytool", "foo", "a", "b", "c"];
        let helper = CompletionHelper::new(&cmd, appendix);
        assert_eq!(helper.get_appendix_of_currently_edited(), vec!["a", "b"],)
    }
    #[test]
    fn test_get_all_of_currently_edited_root() {
        let cmd = Command::new("mytool");
        let appendix = vec!["mytool", "a", "b", "c"];
        let helper = CompletionHelper::new(&cmd, appendix);
        assert_eq!(helper.get_appendix_of_currently_edited(), vec!["a", "b"],)
    }
    #[test]
    fn test_get_all_of_currently_edited_empty() {
        let cmd = Command::new("mytool");
        let appendix = vec!["mytool"];
        let helper = CompletionHelper::new(&cmd, appendix);
        assert_eq!(
            helper.get_appendix_of_currently_edited(),
            Vec::<String>::new(),
        );
        let appendix = vec![];
        let helper = CompletionHelper::new(&cmd, appendix);
        assert_eq!(
            helper.get_appendix_of_currently_edited(),
            Vec::<String>::new(),
        )
    }
}

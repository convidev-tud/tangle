use crate::cli::ArgHelper;
use crate::cli::completion::RelativePathCompleter;
use crate::model::QualifiedPath;
use clap::parser::ValueSource;
use clap::{Arg, ArgAction, ArgMatches, Command};
use std::ops::Range;

#[derive(Debug, Clone)]
pub struct CompletionHelper<'a> {
    command: &'a Command,
    cli_content: Vec<&'a str>,
    arg_matches: ArgMatches,
}
impl<'a> CompletionHelper<'a> {
    pub fn new(root_command: &'a Command, cli_content: Vec<&'a str>) -> Self {
        let arg_matches = root_command
            .clone()
            .ignore_errors(true)
            .get_matches_from(cli_content.clone());
        let mut command = root_command;
        let mut matches = &arg_matches;
        loop {
            match matches.subcommand() {
                Some((name, arg_matches)) => {
                    let maybe_command = command.get_subcommands().find(|c| c.get_name() == name);
                    if maybe_command.is_some() {
                        command = maybe_command.unwrap();
                        matches = arg_matches;
                    } else {
                        break;
                    }
                }
                None => break,
            }
        }
        Self {
            command,
            cli_content,
            arg_matches: matches.clone(),
        }
    }
    pub fn get_last(&self) -> Option<String> {
        Some(self.cli_content.last()?.to_string())
    }

    fn currently_editing_with_range(&self) -> Option<(Option<Range<usize>>, &Arg)> {
        let all_parsed_args = self
            .arg_matches
            .ids()
            .filter(
                |arg| match self.arg_matches.value_source(arg.as_str()).unwrap() {
                    ValueSource::CommandLine => true,
                    _ => false,
                },
            )
            .collect::<Vec<_>>();

        let maybe_last = all_parsed_args.last();
        if maybe_last.is_none() {
            return None;
        }
        let last = *maybe_last.unwrap();
        let last_parsed_arg = self
            .command
            .get_arguments()
            .find(|a| a.get_id() == last)
            .unwrap();
        let indices_of_last_parsed_arg = self
            .arg_matches
            .indices_of(last.as_str())
            .unwrap()
            .collect::<Vec<usize>>();
        let next_positional = self
            .command
            .get_positionals()
            .find(|arg| all_parsed_args.contains(&arg.get_id()));

        match indices_of_last_parsed_arg.len() {
            0 => Some((None, last_parsed_arg)),
            _ => match last_parsed_arg.get_action() {
                ArgAction::Set => {
                    if &ArgHelper::new(self.arg_matches.clone())
                        .get_argument_value::<String>(last_parsed_arg.get_id().as_str())
                        .unwrap()
                        == self.cli_content.last().unwrap()
                    {
                        Some((
                            Some(Range {
                                start: indices_of_last_parsed_arg.first().unwrap().to_owned(),
                                end: indices_of_last_parsed_arg.last().unwrap().to_owned(),
                            }),
                            last_parsed_arg,
                        ))
                    } else if next_positional.is_some() {
                        Some((None, next_positional?))
                    } else {
                        None
                    }
                }
                ArgAction::Append => Some((
                    Some(Range {
                        start: indices_of_last_parsed_arg.first().unwrap().to_owned(),
                        end: indices_of_last_parsed_arg.last().unwrap().to_owned(),
                    }),
                    last_parsed_arg,
                )),
                _ => None,
            },
        }
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
    pub fn get_appendix_of(&self, name: &str) -> Vec<String> {
        let helper = ArgHelper::new(self.arg_matches.clone());
        if !helper.has_arg(name) {
            return vec![];
        }
        helper.get_argument_values(name).unwrap()
    }
    pub fn get_appendix_of_currently_edited(&self) -> Vec<String> {
        let maybe_currently_editing = self.currently_editing_with_range();
        if maybe_currently_editing.is_none() {
            return vec![];
        }
        let currently_editing = maybe_currently_editing.unwrap().1;
        self.get_appendix_of(currently_editing.get_id().as_str())
    }
    pub fn complete_qualified_paths(
        &self,
        reference: QualifiedPath,
        paths: impl Iterator<Item = QualifiedPath>,
    ) -> Vec<String> {
        let maybe_last = self.get_last();
        if maybe_last.is_none() {
            return vec![];
        }
        RelativePathCompleter::new(reference)
            .complete(QualifiedPath::from(maybe_last.unwrap()), paths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_command() -> Command {
        let sub_command = Command::new("sub").arg(Arg::new("sub_option").short('s'));
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
            .subcommand(sub_command)
    }

    #[test]
    fn test_currently_editing_empty() {
        let cmd = setup_test_command();
        let appendix = vec!["mytool"];
        let helper = CompletionHelper::new(&cmd, appendix);
        assert_eq!(helper.currently_editing(), None);
    }
    #[test]
    fn test_currently_editing_one_option_empty() {
        let cmd = setup_test_command();
        let appendix = vec!["mytool", "--option1"];
        let helper = CompletionHelper::new(&cmd, appendix);
        assert_eq!(
            helper.currently_editing().unwrap().get_id().as_str(),
            "option1"
        );
    }
    #[test]
    fn test_currently_editing_one_option_edited() {
        let cmd = setup_test_command();
        let appendix = vec!["mytool", "--option1", "abc"];
        let helper = CompletionHelper::new(&cmd, appendix);
        assert_eq!(
            helper.currently_editing().unwrap().get_id().as_str(),
            "option1"
        );
    }
    #[test]
    fn test_currently_editing_one_option_finished() {
        let cmd = setup_test_command();
        let appendix = vec!["mytool", "--option1", "abc", ""];
        let helper = CompletionHelper::new(&cmd, appendix);
        assert_eq!(
            helper.currently_editing().unwrap().get_id().as_str(),
            "pos1"
        );
    }
    #[test]
    fn test_currently_editing_one_option_one_positional() {
        let cmd = setup_test_command();
        let appendix = vec!["mytool", "--option1", "abc", "foo"];
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
    fn test_currently_editing_subcommands() {
        let cmd = setup_test_command();
        let appendix = vec!["mytool", "-b", "sub", "-s", "abc"];
        let helper = CompletionHelper::new(&cmd, appendix);
        assert_eq!(
            helper.currently_editing().unwrap().get_id().as_str(),
            "sub_option".to_string()
        );
    }
    #[test]
    fn test_get_all_of_currently_edited_except_last() {
        let cmd = setup_test_command();
        let appendix = vec!["mytool", "foo", "a", "b", "c"];
        let helper = CompletionHelper::new(&cmd, appendix);
        assert_eq!(
            helper.get_appendix_of_currently_edited(),
            vec!["a", "b", "c"]
        )
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

use crate::cli::ArgHelper;
use crate::cli::completion::CompletionHelper;
use crate::git::interface::GitInterface;
use crate::model::ImportFormat;
use crate::util::u8_to_string;
use clap::Command;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::process::Output;

#[derive(Debug)]
pub struct CommandMap {
    pub clap_command: Command,
    pub command: Box<dyn CommandInterface>,
    pub children: Vec<CommandMap>,
}

impl CommandMap {
    pub fn new(command: Box<dyn CommandImpl>) -> CommandMap {
        let mut children: Vec<CommandMap> = Vec::new();
        let clap_command = command.build_command().subcommands(
            command
                .get_subcommands()
                .into_iter()
                .map(|c| {
                    let sub_command = c.build_command();
                    children.push(CommandMap::new(c));
                    sub_command
                })
                .collect::<Vec<Command>>(),
        );
        CommandMap {
            clap_command,
            command,
            children,
        }
    }
    pub fn find_child(&self, name: &str) -> Option<&CommandMap> {
        self.children
            .iter()
            .find(|child| child.clap_command.get_name() == name)
    }
    pub fn find_last_child_recursive(&self, names: &mut Vec<&str>) -> Option<&CommandMap> {
        if names.is_empty() {
            return None;
        }
        if names.len() >= 1 && self.clap_command.get_name() == *names.first().unwrap() {
            if names.len() == 1 {
                return Some(self);
            }
            names.remove(0);
            let maybe_child = self.find_child(names.first().unwrap());
            if maybe_child.is_some() {
                let maybe_final = maybe_child.unwrap().find_last_child_recursive(names);
                if maybe_final.is_some() {
                    maybe_final
                } else {
                    Some(self)
                }
            } else {
                Some(self)
            }
        } else {
            None
        }
    }
    pub fn find_children_by_prefix(&self, prefix: &str) -> Vec<&CommandMap> {
        self.children
            .iter()
            .filter(|child| child.clap_command.get_name().starts_with(prefix))
            .collect()
    }
}

#[derive(Debug)]
pub struct CommandContext<'a> {
    pub current_command: &'a CommandMap,
    pub root_command: &'a CommandMap,
    pub git: &'a mut GitInterface,
    pub arg_helper: ArgHelper<'a>,
    pub import_format: ImportFormat,
}

impl CommandContext<'_> {
    pub fn new<'a>(
        current_command: &'a CommandMap,
        root_command: &'a CommandMap,
        git: &'a mut GitInterface,
        arg_helper: ArgHelper<'a>,
        import_format: ImportFormat,
    ) -> CommandContext<'a> {
        CommandContext {
            current_command,
            root_command,
            git,
            arg_helper,
            import_format,
        }
    }
    fn transform_branch_names<S: Into<String>>(&self, to_print: S) -> String {
        let mut result = to_print.into();
        for branch in self.git.get_model().get_qualified_paths_with_branches() {
            result = result.replace(branch.to_git_branch().as_str(), branch.to_string().as_str());
        }
        result
    }
    pub fn log_from_output(&self, output: &Output) {
        self.log_to_stdout(u8_to_string(&output.stdout));
        self.log_to_stderr(u8_to_string(&output.stderr));
    }
    pub fn log_to_stdout<S: Into<String>>(&self, stdout: S) {
        let converted = stdout.into();
        if converted.len() > 0 {
            println!("{}", self.transform_branch_names(converted.trim_end()))
        }
    }
    pub fn log_to_stderr<S: Into<String>>(&self, stderr: S) {
        let converted = stderr.into();
        if converted.len() > 0 {
            println!("{}", self.transform_branch_names(converted.trim_end()))
        }
    }
}

pub trait CommandDefinition: Debug {
    fn build_command(&self) -> Command;
    fn get_subcommands(&self) -> Vec<Box<dyn CommandImpl>> {
        Vec::new()
    }
}

pub trait CommandInterface: Debug {
    fn run_command(&self, _context: &mut CommandContext) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
    fn shell_complete(
        &self,
        _completion_helper: CompletionHelper,
        _context: &mut CommandContext,
    ) -> Result<Vec<String>, Box<dyn Error>> {
        Ok(Vec::new())
    }
}

pub trait CommandImpl: CommandDefinition + CommandInterface + Debug {}
impl<T: CommandDefinition + CommandInterface + Debug> CommandImpl for T {}

#[derive(Debug, Clone)]
pub struct CommandError {
    msg: String,
}
impl CommandError {
    pub fn new(msg: &str) -> CommandError {
        CommandError {
            msg: msg.to_string(),
        }
    }
}
impl Display for CommandError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.msg)
    }
}
impl Error for CommandError {}

use crate::cli::{ArgHelper, CommandContext, CommandImpl, CommandMap, VERBOSE};
use crate::git::interface::{GitInterface, GitPath};
use crate::model::ImportFormat;
use clap::ArgMatches;
use log::LevelFilter;
use std::error::Error;
use std::ffi::OsString;

pub enum ArgSource<'a> {
    CLI,
    SUPPLIED(Vec<&'a str>),
}

#[derive(Debug, Clone)]
pub struct RunStatistics {
    logs: Vec<String>,
}

impl RunStatistics {
    pub fn contains_log<S: Into<String>>(&self, log: S) -> bool {
        self.logs.contains(&log.into())
    }
}

pub struct CommandRepository {
    command_map: CommandMap,
    work_path: GitPath,
}
impl CommandRepository {
    pub fn new(root_command: Box<dyn CommandImpl>, work_path: GitPath) -> Self {
        Self {
            command_map: CommandMap::new(root_command),
            work_path,
        }
    }
    fn execute_recursive<'a>(
        &self,
        mut context: CommandContext<'a>,
    ) -> Result<CommandContext<'a>, Box<dyn Error>> {
        if context.arg_helper.has_arg(VERBOSE) {
            match context.arg_helper.get_count(VERBOSE) {
                0 => log::set_max_level(LevelFilter::Info),
                1 => log::set_max_level(LevelFilter::Debug),
                _ => log::set_max_level(LevelFilter::Trace),
            }
        } else {
            log::set_max_level(LevelFilter::Info)
        }
        let current = context.current_command;
        match current.command.run_command(&mut context) {
            Ok(_) => {}
            Err(err) => return Err(err),
        };
        match context.arg_helper.get_matches().subcommand() {
            Some((sub, sub_args)) => {
                if let Some(child) = current.find_child(sub) {
                    context.current_command = child;
                    context.arg_helper = ArgHelper::new(sub_args.clone());
                    self.execute_recursive(context)
                } else {
                    let ext_args: Vec<_> = sub_args.get_many::<OsString>("").unwrap().collect();
                    let output = std::process::Command::new("git")
                        .arg(sub)
                        .args(ext_args)
                        .output()
                        .expect("failed to execute git");
                    context.log_from_output(&output);
                    Ok(context)
                }
            }
            _ => Ok(context),
        }
    }
    pub fn execute(&self, args: ArgSource) -> Result<(), Box<dyn Error>> {
        let args: ArgMatches = match args {
            ArgSource::CLI => self.command_map.clap_command.clone().get_matches(),
            ArgSource::SUPPLIED(supplied) => self
                .command_map
                .clap_command
                .clone()
                .get_matches_from(supplied),
        };
        let context = CommandContext::new(
            &self.command_map,
            &self.command_map,
            GitInterface::new(self.work_path.clone()),
            ArgHelper::new(args),
            ImportFormat::Native,
        );
        self.execute_recursive(context)?;
        Ok(())
    }
}

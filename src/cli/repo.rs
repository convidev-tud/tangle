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
    fn execute_recursive(&self, context: &mut CommandContext) -> Result<(), Box<dyn Error>> {
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
        match current.command.run_command(context) {
            Ok(_) => {}
            Err(err) => return Err(err),
        };
        match context.arg_helper.get_matches().subcommand() {
            Some((sub, sub_args)) => {
                if let Some(child) = current.find_child(sub) {
                    self.execute_recursive(&mut CommandContext::new(
                        child,
                        context.root_command,
                        context.git,
                        ArgHelper::new(sub_args),
                        context.import_format.clone(),
                    ))
                } else {
                    let ext_args: Vec<_> = sub_args.get_many::<OsString>("").unwrap().collect();
                    let output = std::process::Command::new("git")
                        .arg(sub)
                        .args(ext_args)
                        .output()
                        .expect("failed to execute git");
                    context.log_from_output(&output);
                    Ok(())
                }
            }
            _ => Ok(()),
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
        self.execute_recursive(&mut CommandContext::new(
            &self.command_map,
            &self.command_map,
            &mut GitInterface::new(self.work_path.clone()),
            ArgHelper::new(&args),
            ImportFormat::Native,
        ))?;
        Ok(())
    }
}

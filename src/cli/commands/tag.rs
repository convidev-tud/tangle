use crate::cli::*;
use crate::model::QualifiedPath;
use clap::{Arg, Command};
use std::error::Error;

#[derive(Clone, Debug)]
pub struct TagCommand;

impl CommandDefinition for TagCommand {
    fn build_command(&self) -> Command {
        Command::new("tag")
            .about("Tag a branch")
            .disable_help_subcommand(true)
            .arg(Arg::new("tag").help("The tag to apply to the current branch"))
            .arg(delete(false).help("Delete tag"))
    }
}

impl CommandInterface for TagCommand {
    fn run_command(&self, context: &mut CommandContext) -> Result<(), Box<dyn Error>> {
        let tag = context.arg_helper.get_argument_value::<String>("tag");
        let delete = context.arg_helper.get_argument_value::<String>("delete");

        match delete {
            Some(delete) => {
                let output = context.git.delete_tag(&QualifiedPath::from(delete))?;
                context.log_from_output(&output);
                return Ok(());
            }
            None => {}
        }
        match tag {
            Some(tag) => {
                let output = context.git.create_tag(&QualifiedPath::from(tag))?;
                context.log_from_output(&output);
            }
            None => {
                let current_branch = context.git.get_current_node_path()?;
                let tags = current_branch.get_tags();
                if tags.is_empty() {
                    context.info("No tags on current branch");
                } else {
                    for tag in tags {
                        context.info(tag)
                    }
                }
            }
        }
        Ok(())
    }
}

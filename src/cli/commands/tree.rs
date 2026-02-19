use crate::cli::*;
use clap::Command;
use std::error::Error;

#[derive(Clone, Debug)]
pub struct TreeCommand;

impl CommandDefinition for TreeCommand {
    fn build_command(&self) -> Command {
        Command::new("tree")
            .about("Displays the tree structure")
            .disable_help_subcommand(true)
            .arg(show_tags())
    }
}

impl CommandInterface for TreeCommand {
    fn run_command(&self, context: &mut CommandContext) -> Result<(), Box<dyn Error>> {
        let show_tags = context
            .arg_helper
            .get_argument_value::<bool>("show_tags")
            .unwrap();
        let current_node_path = context.git.get_current_node_path()?;
        let tree = current_node_path.display_tree(show_tags);
        context.info(tree);
        Ok(())
    }
}

use crate::cli::*;
use crate::model::NodePathType;
use clap::Command;
use std::error::Error;

#[derive(Clone, Debug)]
pub struct SpreadCommand;

impl CommandDefinition for SpreadCommand {
    fn build_command(&self) -> Command {
        Command::new("spread")
            .about("Spread commits across children")
            .disable_help_subcommand(true)
    }
}

impl CommandInterface for SpreadCommand {
    fn run_command(&self, context: &mut CommandContext) -> Result<(), Box<dyn Error>> {
        let current_path = context.git.get_current_node_path()?;
        let current_branch = current_path.get_qualified_path();
        let merge_argument = vec![current_branch.clone()];
        for path in current_path.iter_children_req() {
            let qualified_path = path.get_qualified_path();
            match path.concretize() {
                NodePathType::Tag(_) => {}
                _ => {
                    context.info(format!("Spreading to {}", qualified_path));
                    context.git.checkout(&qualified_path)?;
                    context.git.merge(&merge_argument)?;
                }
            }
        }
        context.git.checkout(&current_branch)?;
        context.info("Success");
        Ok(())
    }
}

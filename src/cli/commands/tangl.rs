use std::error::Error;
use crate::cli::*;
use clap::{Arg, Command};
use crate::cli::completion::CompletionHelper;
use crate::model::ImportFormat;

#[derive(Clone, Debug)]
pub struct TangleCommand {}

impl CommandDefinition for TangleCommand {
    fn build_command(&self) -> Command {
        Command::new("tangl")
            .arg_required_else_help(true)
            .allow_external_subcommands(true)
            .arg(
                Arg::new("format")
                    .short('f')
                    .long("import-format")
                    .default_value("native")
                    .help("Specify file import format for all commands")
            )
    }
    fn get_subcommands(&self) -> Vec<Box<dyn CommandImpl>> {
        vec![
            Box::new(StatusCommand),
            Box::new(TreeCommand),
            Box::new(DeriveCommand),
            Box::new(CheckCommand),
            Box::new(CheckoutCommand),
            Box::new(InitCommand),
            Box::new(FeatureCommand),
            Box::new(ProductCommand),
            Box::new(TagCommand),
            Box::new(SpreadCommand),
            Box::new(UntieCommand),
            Box::new(HiddenCompletionCommand),
        ]
    }
}

impl CommandInterface for TangleCommand {
    fn run_command(&self, context: &mut CommandContext) -> Result<(), Box<dyn Error>> {
        let format = context.arg_helper.get_argument_value::<String>("format").unwrap();
        context.import_format = ImportFormat::from(format);
        Ok(())
    }

    fn shell_complete(&self, completion_helper: CompletionHelper, _context: &mut CommandContext) -> Result<Vec<String>, Box<dyn Error>> {
        match completion_helper.currently_editing() {
            Some(value) => {
                match value.get_id().as_str() {
                    "format" => {
                        Ok(vec![
                            "native".to_string(),
                            "waffle".to_string(),
                            "uvl".to_string(),
                        ])
                    }
                    _ => Ok(vec![])
                }
            }
            None => { Ok(vec![]) }
        }
    }
}

use crate::cli::completion::*;
use crate::cli::*;
use crate::model::*;
use clap::{Arg, Command};
use std::error::Error;

fn delete_product(
    product: QualifiedPath,
    context: &mut CommandContext,
) -> Result<(), Box<dyn Error>> {
    let area = context.git.get_current_area()?;
    let complete_path = area.get_path_to_product_root() + product;
    let output = context.git.delete_branch(&complete_path)?;
    context.log_from_output(&output);
    Ok(())
}
fn print_product_tree(context: &mut CommandContext) -> Result<(), Box<dyn Error>> {
    let area = context.git.get_current_area()?;
    match area.to_product_root() {
        Some(path) => {
            context.info(path.display_tree(false));
        }
        None => {}
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct ProductCommand;
impl CommandDefinition for ProductCommand {
    fn build_command(&self) -> Command {
        Command::new("product")
            .about("Manage products")
            .disable_help_subcommand(true)
            .arg(
                Arg::new("delete")
                    .short('D')
                    .help("Deletes a product branch"),
            )
    }
}
impl CommandInterface for ProductCommand {
    fn run_command(&self, context: &mut CommandContext) -> Result<(), Box<dyn Error>> {
        let maybe_delete = context.arg_helper.get_argument_value::<String>("delete");
        match maybe_delete {
            Some(delete) => {
                delete_product(QualifiedPath::from(delete), context)?;
                Ok(())
            }
            None => {
                print_product_tree(context)?;
                Ok(())
            }
        }
    }
    fn shell_complete(
        &self,
        completion_helper: CompletionHelper,
        context: &mut CommandContext,
    ) -> Result<Vec<String>, Box<dyn Error>> {
        let result = match completion_helper.currently_editing() {
            Some(arg) => match arg.get_id().as_str() {
                "delete" => {
                    let maybe_feature_root = context.git.get_current_area()?.to_product_root();
                    match maybe_feature_root {
                        Some(path) => completion_helper.complete_qualified_paths(
                            path.get_qualified_path(),
                            HasBranchFilteringNodePathTransformer::new(true)
                                .transform(path.iter_children_req())
                                .map(|path| path.get_qualified_path()),
                        ),
                        None => {
                            vec![]
                        }
                    }
                }
                _ => {
                    vec![]
                }
            },
            None => {
                vec![]
            }
        };
        Ok(result)
    }
}

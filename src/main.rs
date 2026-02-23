use log::{LevelFilter, error, set_logger, set_max_level};
use tangl::cli::{ArgSource, CommandRepository, TangleCommand};
use tangl::git::interface::GitPath;
use tangl::logging::PrintingLogger;

fn main() {
    set_logger(&PrintingLogger).unwrap();
    set_max_level(LevelFilter::Info);
    let command_repository =
        CommandRepository::new(Box::new(TangleCommand {}), GitPath::CurrentDirectory);
    match command_repository.execute(ArgSource::CLI) {
        Ok(_) => {}
        Err(error) => {
            error!("{}", error)
        }
    }
}

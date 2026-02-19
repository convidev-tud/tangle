use log::{error, max_level, set_logger, set_max_level, LevelFilter, Log, Metadata, Record};
use tangl::cli::{ArgSource, CommandRepository, TangleCommand};
use tangl::git::interface::GitPath;

struct BinaryLogger;
impl Log for BinaryLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= max_level()
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            println!("{}", record.args());
        }
    }

    fn flush(&self) {}
}

fn main() {
    set_logger(&BinaryLogger).unwrap();
    set_max_level(LevelFilter::Info);
    let command_repository =
        CommandRepository::new(Box::new(TangleCommand {}), GitPath::CurrentDirectory);
    match command_repository.execute(ArgSource::CLI) {
        Ok(_) => {}
        Err(error) => { error!("{}", error) }
    }
}

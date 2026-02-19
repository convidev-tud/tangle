use log::{Log, Metadata, Record, max_level};
use std::sync::Mutex;

pub struct PrintingLogger;
impl Log for PrintingLogger {
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

pub struct CollectingLogger {
    logs: Mutex<Vec<String>>,
}
impl Log for CollectingLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= max_level()
    }

    fn log(&self, record: &Record) {
        self.logs.lock().unwrap().push(format!("{}", record.args()));
    }

    fn flush(&self) {}
}

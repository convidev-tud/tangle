use crate::git::interface::GitCLI;
use crate::util::u8_to_string;
use std::error::Error;
use std::fs::{read_to_string, write};

trait PersistencyHandler<E> {
    fn read_file(&self) -> Result<String, E>;
    fn write_file(&self, data: &str) -> Result<(), E>;
}

pub struct GitDirPersistencyHandler {
    file_path: String,
    raw_git_interface: GitCLI,
}

impl GitDirPersistencyHandler {
    pub fn new(file_name: &str) -> Self {
        let path = String::from("tangl/") + file_name;
        Self {
            file_path: path,
            raw_git_interface: GitCLI::in_current_directory(),
        }
    }
    fn get_file_path(&self) -> String {
        let maybe_output = self
            .raw_git_interface
            .run(vec!["rev-parse", "--show-toplevel"]);
        match maybe_output {
            Ok(output) => u8_to_string(&output.stdout),
            Err(error) => {
                panic!("{}", error)
            }
        }
    }
}

impl PersistencyHandler<Box<dyn Error>> for GitDirPersistencyHandler {
    fn read_file(&self) -> Result<String, Box<dyn Error>> {
        Ok(read_to_string(&self.get_file_path())?)
    }

    fn write_file(&self, data: &str) -> Result<(), Box<dyn Error>> {
        Ok(write(self.get_file_path(), data)?)
    }
}

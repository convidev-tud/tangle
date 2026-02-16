use crate::model::{QualifiedPath, TreeDataModel, WrongNodeTypeError};

#[derive(Debug, Clone)]
pub enum ImportFormat {
    Native,
    Waffle,
    UVL,
}

impl From<String> for ImportFormat {
    fn from(value: String) -> Self {
        Self::from(value.as_str())
    }
}

impl From<&str> for ImportFormat {
    fn from(value: &str) -> Self {
        match value.to_uppercase().as_str() {
            "NATIVE" => ImportFormat::Native,
            "WAFFLE" => ImportFormat::Waffle,
            "UVL" => ImportFormat::UVL,
            _ => unreachable!("Importer does not support format '{}'", value),
        }
    }
}

pub trait FormatParser {
    fn parse(&self, data: &str) -> Vec<QualifiedPath>;
}

pub struct ModelImporter {
    parser: Box<dyn FormatParser>,
}

impl ModelImporter {
    pub fn new(format: ImportFormat) -> ModelImporter {
        let parser = match format {
            ImportFormat::Waffle => WaffleImporter,
            _ => {
                todo!()
            }
        };
        ModelImporter {
            parser: Box::new(parser),
        }
    }
    pub fn import(&self, data: &str) -> Result<TreeDataModel, WrongNodeTypeError> {
        let paths = self.parser.parse(&data);
        let mut model = TreeDataModel::new();
        for path in paths {
            model.insert_qualified_path(path, false)?;
        }
        Ok(model)
    }
}

pub struct WaffleImporter;

impl FormatParser for WaffleImporter {
    fn parse(&self, data: &str) -> Vec<QualifiedPath> {
        todo!()
    }
}

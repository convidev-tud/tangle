use crate::model::*;

pub const FEATURES_PREFIX: &str = "feature";
pub const PRODUCTS_PREFIX: &str = "product";

#[derive(Clone, Debug)]
pub struct TreeDataModel {
    virtual_root: Node,
    qualified_paths_with_branch: Vec<QualifiedPath>,
}
impl TreeDataModel {
    pub fn new() -> Self {
        Self {
            virtual_root: Node::new("", NodeType::VirtualRoot, NodeMetadata::default()),
            qualified_paths_with_branch: vec![],
        }
    }
    pub fn insert_qualified_path(
        &mut self,
        path: QualifiedPath,
        is_tag: bool,
    ) -> Result<(), WrongNodeTypeError> {
        self.virtual_root
            .insert_node_path(&path, NodeMetadata::new(true), is_tag)?;
        self.qualified_paths_with_branch.push(path);
        Ok(())
    }
    pub fn get_area(&self, path: &QualifiedPath) -> Option<NodePath<Area>> {
        Some(NodePath::<Area>::new(
            vec![self.virtual_root.get_child(path.first()?)?.clone()],
        ))
    }
    pub fn get_node_path(&self, path: &QualifiedPath) -> Option<NodePath<AnyNodeType>> {
        let initial_path = self.get_area(&path.first()?)?;
        let new_path = path.strip_n_left(1);
        initial_path.to(&new_path)
    }
    pub fn has_branch(&self, qualified_path: &QualifiedPath) -> bool {
        self.qualified_paths_with_branch
            .iter()
            .find(|e| *e == qualified_path)
            .is_some()
    }
    pub fn get_qualified_paths_with_branches(&self) -> &Vec<QualifiedPath> {
        &self.qualified_paths_with_branch
    }
}

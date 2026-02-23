use crate::model::*;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::rc::Rc;

pub trait NodePathBasicNavigation
where
    Self: Sized,
{
    fn to(self, path: &QualifiedPath) -> Option<NodePath<AnyNodeType>>;
    fn to_last_valid(self, path: &QualifiedPath) -> NodePath<AnyNodeType>;
}
pub trait NodePathFeatureNavigation: NodePathBasicNavigation
where
    Self: Sized,
{
    fn to_feature(self, path: &QualifiedPath) -> Option<NodePath<Feature>> {
        match self.to(path)?.concretize() {
            NodePathType::Feature(path) => Some(path),
            _ => unreachable!(),
        }
    }
}
pub trait NodePathProductNavigation: NodePathBasicNavigation
where
    Self: Sized,
{
    fn to_product(self, path: &QualifiedPath) -> Option<NodePath<Product>> {
        match self.to(path)?.concretize() {
            NodePathType::Product(path) => Some(path),
            _ => unreachable!(),
        }
    }
}

pub enum NodePathType {
    Feature(NodePath<Feature>),
    FeatureRoot(NodePath<FeatureRoot>),
    Product(NodePath<Product>),
    ProductRoot(NodePath<ProductRoot>),
    Area(NodePath<Area>),
    VirtualRoot(NodePath<VirtualRoot>),
    Tag(NodePath<Tag>),
}

#[derive(Clone, Debug)]
pub struct NodePath<T: Clone + Debug> {
    path: Vec<Rc<Node>>,
    _phantom: PhantomData<T>,
}

impl NodePath<AnyNodeType> {
    fn to_concrete_type<T: Clone + Debug>(self) -> NodePath<T> {
        NodePath::<T>::new(self.path)
    }
    pub fn from_concrete<T: Clone + Debug>(other: NodePath<T>) -> Self {
        Self::new(other.path)
    }
    pub fn concretize(self) -> NodePathType {
        match self.get_node().get_type() {
            NodeType::Feature => NodePathType::Feature(self.to_concrete_type()),
            NodeType::FeatureRoot => NodePathType::FeatureRoot(self.to_concrete_type()),
            NodeType::Product => NodePathType::Product(self.to_concrete_type()),
            NodeType::ProductRoot => NodePathType::ProductRoot(self.to_concrete_type()),
            NodeType::Area => NodePathType::Area(self.to_concrete_type()),
            NodeType::VirtualRoot => NodePathType::VirtualRoot(self.to_concrete_type()),
            NodeType::Tag => NodePathType::Tag(self.to_concrete_type()),
        }
    }
}

impl NodePath<VirtualRoot> {
    pub fn to_area(self, area: &QualifiedPath) -> Option<NodePath<Area>> {
        match self.to(area)?.concretize() {
            NodePathType::Area(area) => Some(area),
            _ => unreachable!(),
        }
    }
}

impl NodePath<Area> {
    pub fn get_path_to_feature_root(&self) -> QualifiedPath {
        self.get_qualified_path() + QualifiedPath::from(FEATURES_PREFIX)
    }
    pub fn get_path_to_product_root(&self) -> QualifiedPath {
        self.get_qualified_path() + QualifiedPath::from(PRODUCTS_PREFIX)
    }
    pub fn to_feature_root(self) -> Option<NodePath<FeatureRoot>> {
        match self.to(&QualifiedPath::from(FEATURES_PREFIX))?.concretize() {
            NodePathType::FeatureRoot(path) => Some(path),
            _ => unreachable!(),
        }
    }
    pub fn to_product_root(self) -> Option<NodePath<ProductRoot>> {
        match self.to(&QualifiedPath::from(PRODUCTS_PREFIX))?.concretize() {
            NodePathType::ProductRoot(path) => Some(path),
            _ => unreachable!(),
        }
    }
}

impl NodePathProductNavigation for NodePath<ProductRoot> {}
impl NodePathProductNavigation for NodePath<Product> {}

impl NodePathFeatureNavigation for NodePath<FeatureRoot> {}
impl NodePathFeatureNavigation for NodePath<Feature> {}

impl<T: Clone + Debug> NodePath<T> {
    fn get_node(&self) -> &Node {
        self.path.last().unwrap()
    }
    pub fn new(path: Vec<Rc<Node>>) -> NodePath<T> {
        Self {
            path,
            _phantom: PhantomData,
        }
    }
    pub fn iter_children(&self) -> impl Iterator<Item = NodePath<AnyNodeType>> {
        self.get_node()
            .iter_children()
            .map(|(name, _)| self.clone().to(&QualifiedPath::from(name.clone())).unwrap())
    }
    pub fn iter_children_req(&self) -> impl Iterator<Item = NodePath<AnyNodeType>> {
        self.iter_children().flat_map(|path| {
            let mut to_iter = Vec::new();
            to_iter.push(path.clone());
            to_iter.extend(path.iter_children_req());
            to_iter
        })
    }
    pub fn get_tags(&self) -> Vec<QualifiedPath> {
        self.get_node()
            .iter_children()
            .filter_map(|(name, child)| match child.get_type() {
                NodeType::Tag => Some(QualifiedPath::from(name.clone())),
                _ => None,
            })
            .collect()
    }
    pub fn get_metadata(&self) -> &NodeMetadata {
        self.get_node().get_metadata()
    }
    pub fn transform_to_any_type(self) -> NodePath<AnyNodeType> {
        NodePath::<AnyNodeType>::from_concrete(self)
    }
    pub fn get_qualified_path(&self) -> QualifiedPath {
        let mut path = QualifiedPath::new();
        for p in self.path.iter() {
            path.push(p.get_name());
        }
        path
    }
    pub fn display_tree(&self, show_tags: bool) -> String {
        self.get_node().display_tree(show_tags)
    }
}

impl<T: Clone + Debug> NodePathBasicNavigation for NodePath<T> {
    fn to(mut self, path: &QualifiedPath) -> Option<NodePath<AnyNodeType>> {
        for p in path.iter_string() {
            self.path.push(self.get_node().get_child(p)?.clone());
        }
        Some(NodePath::<AnyNodeType>::new(self.path))
    }

    fn to_last_valid(self, path: &QualifiedPath) -> NodePath<AnyNodeType> {
        let mut current = self.transform_to_any_type();
        for part in path.iter() {
            let next = current.clone().to(&part);
            if next.is_some() {
                current = next.unwrap();
            } else {
                break;
            }
        }
        current
    }
}

pub trait NodePathTransformer<A, B>
where
    A: Clone + Debug,
    B: Clone + Debug,
{
    fn apply(&self, node_path: NodePath<A>) -> Option<NodePath<B>>;
    fn transform(
        &self,
        node_paths: impl Iterator<Item = NodePath<A>>,
    ) -> impl Iterator<Item = NodePath<B>> {
        node_paths.filter_map(|path| self.apply(path))
    }
}

pub enum NodePathTransformers {
    ChainingNodePathTransformer(ChainingNodePathTransformer),
    HasBranchFilteringNodePathTransformer(HasBranchFilteringNodePathTransformer),
    ByQPathFilteringNodePathTransformer(ByQPathFilteringNodePathTransformer),
}
impl NodePathTransformer<AnyNodeType, AnyNodeType> for NodePathTransformers {
    fn apply(&self, node_path: NodePath<AnyNodeType>) -> Option<NodePath<AnyNodeType>> {
        match self {
            NodePathTransformers::ChainingNodePathTransformer(t) => t.apply(node_path),
            NodePathTransformers::HasBranchFilteringNodePathTransformer(t) => t.apply(node_path),
            NodePathTransformers::ByQPathFilteringNodePathTransformer(t) => t.apply(node_path),
        }
    }
}

pub struct ChainingNodePathTransformer {
    transformers: Vec<NodePathTransformers>,
}
impl ChainingNodePathTransformer {
    pub fn new(transformers: Vec<NodePathTransformers>) -> Self {
        Self { transformers }
    }
}
impl NodePathTransformer<AnyNodeType, AnyNodeType> for ChainingNodePathTransformer {
    fn apply(&self, node_path: NodePath<AnyNodeType>) -> Option<NodePath<AnyNodeType>> {
        let mut result: Option<NodePath<AnyNodeType>> = Some(node_path);
        for transformer in self.transformers.iter() {
            result = Some(transformer.apply(result.take()?)?);
        }
        result
    }
}

pub struct HasBranchFilteringNodePathTransformer {
    has_branch: bool,
}
impl HasBranchFilteringNodePathTransformer {
    pub fn new(has_branch: bool) -> HasBranchFilteringNodePathTransformer {
        Self { has_branch }
    }
}
impl<A: Clone + Debug> NodePathTransformer<A, A> for HasBranchFilteringNodePathTransformer {
    fn apply(&self, node_path: NodePath<A>) -> Option<NodePath<A>> {
        if node_path.get_metadata().has_branch() == self.has_branch {
            Some(node_path)
        } else {
            None
        }
    }
}

pub enum QPathFilteringMode {
    INCLUDE,
    EXCLUDE,
}
pub struct ByQPathFilteringNodePathTransformer {
    paths: Vec<QualifiedPath>,
    mode: QPathFilteringMode,
}
impl ByQPathFilteringNodePathTransformer {
    pub fn new(paths: Vec<QualifiedPath>, mode: QPathFilteringMode) -> Self {
        Self { paths, mode }
    }
}
impl<A: Clone + Debug> NodePathTransformer<A, A> for ByQPathFilteringNodePathTransformer {
    fn apply(&self, node_path: NodePath<A>) -> Option<NodePath<A>> {
        match self.mode {
            QPathFilteringMode::INCLUDE => {
                if self.paths.contains(&node_path.get_qualified_path()) {
                    Some(node_path)
                } else {
                    None
                }
            }
            QPathFilteringMode::EXCLUDE => {
                if self.paths.contains(&node_path.get_qualified_path()) {
                    None
                } else {
                    Some(node_path)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prepare_model() -> TreeDataModel {
        let mut model = TreeDataModel::new();
        model
            .insert_qualified_path(QualifiedPath::from("/main/feature/root"), false)
            .unwrap();
        model
            .insert_qualified_path(QualifiedPath::from("/main/feature/root/foo"), false)
            .unwrap();
        model
    }

    #[test]
    fn test_chaining_node_path_transformer() {
        let model = prepare_model();
        let chain = ChainingNodePathTransformer::new(vec![
            NodePathTransformers::ByQPathFilteringNodePathTransformer(
                ByQPathFilteringNodePathTransformer::new(
                    vec![QualifiedPath::from("/main/feature/root")],
                    QPathFilteringMode::EXCLUDE,
                ),
            ),
            NodePathTransformers::HasBranchFilteringNodePathTransformer(
                HasBranchFilteringNodePathTransformer::new(true),
            ),
        ]);
        let root = model.get_virtual_root();
        let actual = chain
            .transform(root.iter_children_req())
            .map(|node_path| node_path.get_qualified_path())
            .collect::<Vec<_>>();
        assert_eq!(actual, vec!["/main/feature/root/foo"]);
    }

    #[test]
    fn test_q_path_filtering_node_path_transformer_include() {
        let model = prepare_model();
        let transformer = ByQPathFilteringNodePathTransformer::new(
            vec![QualifiedPath::from("/main/feature/root")],
            QPathFilteringMode::INCLUDE,
        );
        let root = model.get_virtual_root();
        let actual = transformer
            .transform(root.iter_children_req())
            .map(|node_path| node_path.get_qualified_path())
            .collect::<Vec<_>>();
        assert_eq!(actual, vec!["/main/feature/root"]);
    }

    #[test]
    fn test_q_path_filtering_node_path_transformer_exclude() {
        let model = prepare_model();
        let transformer = ByQPathFilteringNodePathTransformer::new(
            vec![QualifiedPath::from("/main/feature/root")],
            QPathFilteringMode::EXCLUDE,
        );
        let root = model.get_virtual_root();
        let actual = transformer
            .transform(root.iter_children_req())
            .map(|node_path| node_path.get_qualified_path())
            .collect::<Vec<_>>();
        assert_eq!(
            actual,
            vec!["/main", "/main/feature", "/main/feature/root/foo"]
        );
    }
}

use crate::model::Node;
pub type AnyResult<T> = anyhow::Result<T, anyhow::Error>;
// NodeList 是 Node 的向量
pub type NodeList = Vec<Node>;

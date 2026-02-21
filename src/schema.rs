//! Schema tree for JSON leaf de-duplication.

use std::fmt;
use std::io;

use fxhash::FxHashMap;
use serde_json_borrow::{OwnedValue, Value};

/// A unique identifier for a leaf (path + kind) in the schema tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LeafId(pub u32);

/// A list of leaf ids that uniquely identifies a schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaId(pub Vec<LeafId>);

impl SchemaId {
    /// Create a SchemaId from a list of leaf ids.
    pub fn new(mut ids: Vec<LeafId>) -> Self {
        ids.sort_unstable();
        ids.dedup();
        Self(ids)
    }

    /// Returns a reference to the contained leaf ids.
    pub fn leaf_ids(&self) -> &[LeafId] {
        &self.0
    }

    /// Reconstruct leaf infos for this schema id from the given tree.
    pub fn reconstruct<'a>(&self, tree: &'a SchemaTree) -> Vec<&'a LeafInfo> {
        self.0.iter().filter_map(|id| tree.leaf_info(*id)).collect()
    }
}

/// The kind of a JSON leaf value.
///
/// Arrays are treated as leaves; their contents are not traversed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LeafKind {
    /// JSON null.
    Null,
    /// JSON boolean.
    Bool,
    /// JSON number.
    Number,
    /// JSON string.
    String,
    /// JSON array (arrays are treated as leaves).
    Array,
}

impl LeafKind {
    const COUNT: usize = 5;

    fn index(self) -> usize {
        match self {
            LeafKind::Null => 0,
            LeafKind::Bool => 1,
            LeafKind::Number => 2,
            LeafKind::String => 3,
            LeafKind::Array => 4,
        }
    }
}

/// Information about a leaf in the schema tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeafInfo {
    /// The JSON path from the root object to the leaf.
    pub path: Vec<String>,
    /// The kind of the leaf value.
    pub kind: LeafKind,
}

/// Errors returned by schema parsing and ingestion.
#[derive(Debug)]
pub enum SchemaError {
    /// The JSON failed to parse.
    Parse(io::Error),
    /// The JSON root is not an object.
    RootNotObject,
}

impl fmt::Display for SchemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SchemaError::Parse(err) => write!(f, "failed to parse JSON: {err}"),
            SchemaError::RootNotObject => write!(f, "root JSON value is not an object"),
        }
    }
}

impl std::error::Error for SchemaError {}

impl From<io::Error> for SchemaError {
    fn from(err: io::Error) -> Self {
        SchemaError::Parse(err)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct NodeId(u32);

const ROOT_NODE_ID: NodeId = NodeId(0);

#[derive(Debug, Default)]
struct SchemaNode {
    parent: Option<NodeId>,
    segment: String,
    children: FxHashMap<String, NodeId>,
    leaves: [Option<LeafId>; LeafKind::COUNT],
}

/// A schema tree that de-duplicates leaf paths and assigns leaf ids.
#[derive(Debug)]
pub struct SchemaTree {
    nodes: Vec<SchemaNode>,
    leaves: Vec<LeafInfo>,
}

impl Default for SchemaTree {
    fn default() -> Self {
        Self {
            nodes: vec![SchemaNode::default()],
            leaves: Vec::new(),
        }
    }
}

impl SchemaTree {
    /// Create an empty schema tree.
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse JSON and return its SchemaId, de-duplicating leaf ids in the tree.
    pub fn ingest_json(&mut self, json: &str) -> Result<SchemaId, SchemaError> {
        let value: serde_json_borrow::Value = serde_json::from_str(json)
            .map_err(|err| SchemaError::Parse(io::Error::new(io::ErrorKind::InvalidData, err)))?;
        self.ingest_value(&value)
    }

    /// Parse JSON and return its SchemaId, invoking a callback for each leaf.
    pub fn ingest_json_with<F>(&mut self, json: &str, on_leaf: F) -> Result<SchemaId, SchemaError>
    where
        F: FnMut(LeafId, &Value),
    {
        let owned = OwnedValue::from_str(json)?;
        self.ingest_value_with(owned.get_value(), on_leaf)
    }

    /// Ingest a parsed JSON value and return its SchemaId.
    ///
    /// The root value must be an object.
    pub fn ingest_value(&mut self, value: &Value) -> Result<SchemaId, SchemaError> {
        self.ingest_value_with(value, |_, _| {})
    }

    /// Ingest a parsed JSON value and return its SchemaId, invoking a callback for each leaf.
    ///
    /// The callback receives the leaf id and the original value at that leaf.
    pub fn ingest_value_with<F>(
        &mut self,
        value: &Value,
        mut on_leaf: F,
    ) -> Result<SchemaId, SchemaError>
    where
        F: FnMut(LeafId, &Value),
    {
        match value {
            Value::Object(obj) => {
                let mut leaf_ids = Vec::with_capacity(32);
                self.walk_object(obj, ROOT_NODE_ID, &mut leaf_ids, &mut on_leaf);
                Ok(SchemaId::new(leaf_ids))
            }
            _ => Err(SchemaError::RootNotObject),
        }
    }

    /// Lookup leaf information for a given leaf id.
    pub fn leaf_info(&self, id: LeafId) -> Option<&LeafInfo> {
        self.leaves.get(id.0 as usize)
    }

    /// Return the number of unique leaves tracked by the schema tree.
    pub fn leaf_count(&self) -> usize {
        self.leaves.len()
    }

    fn walk_object<F>(
        &mut self,
        obj: &serde_json_borrow::ObjectAsVec<'_>,
        node_id: NodeId,
        out: &mut Vec<LeafId>,
        on_leaf: &mut F,
    ) where
        F: FnMut(LeafId, &Value),
    {
        for (key, value) in obj.iter() {
            let child_id = self.get_or_create_child(node_id, key);
            self.walk_value(value, child_id, out, on_leaf);
        }
    }

    fn walk_value<F>(
        &mut self,
        value: &Value,
        node_id: NodeId,
        out: &mut Vec<LeafId>,
        on_leaf: &mut F,
    ) where
        F: FnMut(LeafId, &Value),
    {
        match value {
            Value::Object(obj) => self.walk_object(obj, node_id, out, on_leaf),
            Value::Array(_) => self.emit_leaf(node_id, LeafKind::Array, value, out, on_leaf),
            Value::Null => self.emit_leaf(node_id, LeafKind::Null, value, out, on_leaf),
            Value::Bool(_) => self.emit_leaf(node_id, LeafKind::Bool, value, out, on_leaf),
            Value::Number(_) => self.emit_leaf(node_id, LeafKind::Number, value, out, on_leaf),
            Value::Str(_) => self.emit_leaf(node_id, LeafKind::String, value, out, on_leaf),
        }
    }

    fn emit_leaf<F>(
        &mut self,
        node_id: NodeId,
        kind: LeafKind,
        value: &Value,
        out: &mut Vec<LeafId>,
        on_leaf: &mut F,
    ) where
        F: FnMut(LeafId, &Value),
    {
        let id = self.intern_leaf(node_id, kind);
        out.push(id);
        on_leaf(id, value);
    }

    fn get_or_create_child(&mut self, parent_id: NodeId, key: &str) -> NodeId {
        let parent_index = parent_id.0 as usize;
        if let Some(&child_id) = self.nodes[parent_index].children.get(key) {
            return child_id;
        }

        let child_id = NodeId(self.nodes.len() as u32);
        let segment = key.to_string();
        self.nodes.push(SchemaNode {
            parent: Some(parent_id),
            segment: segment.clone(),
            ..Default::default()
        });
        self.nodes[parent_index].children.insert(segment, child_id);
        child_id
    }

    fn node_path(&self, mut node_id: NodeId) -> Vec<String> {
        let mut path_rev = Vec::new();
        loop {
            let node = &self.nodes[node_id.0 as usize];
            if !node.segment.is_empty() {
                path_rev.push(node.segment.clone());
            }
            match node.parent {
                Some(parent_id) => node_id = parent_id,
                None => break,
            }
        }
        path_rev.reverse();
        path_rev
    }

    fn intern_leaf(&mut self, node_id: NodeId, kind: LeafKind) -> LeafId {
        let node_index = node_id.0 as usize;
        if let Some(existing) = self.nodes[node_index].leaves[kind.index()] {
            return existing;
        }

        let id = LeafId(self.leaves.len() as u32);
        self.nodes[node_index].leaves[kind.index()] = Some(id);
        self.leaves.push(LeafInfo {
            path: self.node_path(node_id),
            kind,
        });
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedups_leaves_across_documents() {
        let mut tree = SchemaTree::new();
        let schema_id_1 = tree.ingest_json(r#"{"a": 1, "b": {"c": "x"}}"#).unwrap();
        let schema_id_2 = tree.ingest_json(r#"{"a": 2, "b": {"c": "y"}}"#).unwrap();

        assert_eq!(schema_id_1, schema_id_2);
        assert_eq!(tree.leaf_count(), 2);
    }

    #[test]
    fn same_path_different_kind_gets_distinct_leaf_ids() {
        let mut tree = SchemaTree::new();
        let schema_id_1 = tree.ingest_json(r#"{"a": 1}"#).unwrap();
        let schema_id_2 = tree.ingest_json(r#"{"a": "str"}"#).unwrap();

        assert_ne!(schema_id_1, schema_id_2);
        assert_eq!(tree.leaf_count(), 2);
    }

    #[test]
    fn rejects_non_object_root() {
        let mut tree = SchemaTree::new();
        let err = tree.ingest_json(r#"[1,2,3]"#).unwrap_err();
        assert!(matches!(err, SchemaError::RootNotObject));
    }

    #[test]
    fn callback_reports_leaf_values() {
        let mut tree = SchemaTree::new();
        let mut seen = Vec::new();
        let schema_id = tree
            .ingest_json_with(r#"{"a": true}"#, |leaf_id, value| {
                seen.push((leaf_id, value.to_string()));
            })
            .unwrap();

        assert_eq!(schema_id.leaf_ids().len(), 1);
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].1, "true");
    }

    #[test]
    fn reconstructs_leaf_path() {
        let mut tree = SchemaTree::new();
        let schema_id = tree.ingest_json(r#"{"a": {"b": 1}}"#).unwrap();
        let leaf_info = tree.leaf_info(schema_id.leaf_ids()[0]).unwrap();

        assert_eq!(leaf_info.path, vec!["a".to_string(), "b".to_string()]);
    }
}

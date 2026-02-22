//! Schema tree for JSON leaf de-duplication.

use std::borrow::Cow;
use std::fmt;
use std::io;

use fxhash::FxHashMap;
use serde::Deserialize;
use serde::de::{DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde_json::{Map as JsonMap, Value as JsonValue};
use serde_json_borrow::Value;

/// A unique identifier for a leaf in the schema tree.
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
        self.0
            .iter()
            .map(|leaf_id| tree.leaf_info(*leaf_id))
            .collect()
    }

    /// Reconstruct a JSON object value for this schema using placeholder leaf values.
    ///
    /// Placeholder values are selected by leaf kind:
    /// - null => `null`
    /// - bool => `false`
    /// - number => `0`
    /// - string => `""`
    /// - array => `[]`
    pub fn reconstruct_object(&self, tree: &SchemaTree) -> JsonValue {
        tree.reconstruct_object_with(self, &mut |_, leaf_info| leaf_info.kind.placeholder_value())
    }

    /// Reconstruct this schema into a serialized JSON object string with placeholder leaf values.
    pub fn reconstruct_json(&self, tree: &SchemaTree) -> String {
        serde_json::to_string(&self.reconstruct_object(tree))
            .expect("serializing reconstructed JSON object should not fail")
    }

    /// Reconstruct this schema into a serialized JSON object string with caller-provided leaf values.
    ///
    /// The callback is invoked once for each leaf present in this schema id.
    pub fn reconstruct_json_with<F>(&self, tree: &SchemaTree, mut leaf_value_for: F) -> String
    where
        F: FnMut(LeafId, &LeafInfo) -> JsonValue,
    {
        let reconstructed = tree.reconstruct_object_with(self, &mut leaf_value_for);
        serde_json::to_string(&reconstructed)
            .expect("serializing reconstructed JSON object should not fail")
    }

    fn contains_leaf_id(&self, leaf_id: LeafId) -> bool {
        self.0.binary_search(&leaf_id).is_ok()
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

    fn placeholder_value(self) -> JsonValue {
        match self {
            LeafKind::Null => JsonValue::Null,
            LeafKind::Bool => JsonValue::Bool(false),
            LeafKind::Number => JsonValue::from(0),
            LeafKind::String => JsonValue::String(String::new()),
            LeafKind::Array => JsonValue::Array(Vec::new()),
        }
    }
}

/// Information about a leaf in the schema tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeafInfo {
    /// The leaf key (last segment of the JSON path).
    pub key: String,
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
    /// Normally, we have less than 10 children per node, so we could use a vector.
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
        self.ingest_json_with(json, |_, _| {})
    }

    /// Parse JSON and return its SchemaId, invoking a callback for each leaf.
    pub fn ingest_json_with<F>(
        &mut self,
        json: &str,
        mut on_leaf: F,
    ) -> Result<SchemaId, SchemaError>
    where
        F: FnMut(LeafId, &Value),
    {
        let mut leaf_ids = Vec::with_capacity(32);
        let mut deserializer = serde_json::Deserializer::from_str(json);
        let object_parse_result = ObjectSeedWithCallback {
            tree: self,
            node_id: ROOT_NODE_ID,
            out: &mut leaf_ids,
            on_leaf: &mut on_leaf,
        }
        .deserialize(&mut deserializer);
        if object_parse_result.is_ok() {
            deserializer.end().map_err(Self::parse_error)?;
            return Ok(SchemaId::new(leaf_ids));
        }

        let mut non_object_deserializer = serde_json::Deserializer::from_str(json);
        IgnoredAny::deserialize(&mut non_object_deserializer).map_err(Self::parse_error)?;
        non_object_deserializer.end().map_err(Self::parse_error)?;
        Err(SchemaError::RootNotObject)
    }

    /// Lookup leaf information for a given leaf id.
    pub fn leaf_info(&self, id: LeafId) -> &LeafInfo {
        &self.leaves[id.0 as usize]
    }

    /// Return the number of unique leaves tracked by the schema tree.
    pub fn leaf_count(&self) -> usize {
        self.leaves.len()
    }

    fn reconstruct_object_with<F>(&self, schema_id: &SchemaId, leaf_value_for: &mut F) -> JsonValue
    where
        F: FnMut(LeafId, &LeafInfo) -> JsonValue,
    {
        for leaf_id in schema_id.leaf_ids() {
            let _ = self.leaf_info(*leaf_id);
        }

        let mut root_object = JsonMap::new();
        for (child_key, child_id) in self.sorted_children(ROOT_NODE_ID) {
            if let Some(child_value) =
                self.reconstruct_node_value_with(schema_id, child_id, leaf_value_for)
            {
                root_object.insert(child_key.to_string(), child_value);
            }
        }
        JsonValue::Object(root_object)
    }

    fn reconstruct_node_value_with<F>(
        &self,
        schema_id: &SchemaId,
        node_id: NodeId,
        leaf_value_for: &mut F,
    ) -> Option<JsonValue>
    where
        F: FnMut(LeafId, &LeafInfo) -> JsonValue,
    {
        let node = &self.nodes[node_id.0 as usize];
        let mut selected_leaf_id = None;
        for leaf_id in node.leaves.iter().flatten().copied() {
            if schema_id.contains_leaf_id(leaf_id) {
                if let Some(previous_leaf_id) = selected_leaf_id {
                    panic!(
                        "schema id contains multiple leaf kinds for one path: {:?} and {:?}",
                        previous_leaf_id, leaf_id
                    );
                }
                selected_leaf_id = Some(leaf_id);
            }
        }

        let mut child_object = JsonMap::new();
        for (child_key, child_id) in self.sorted_children(node_id) {
            if let Some(child_value) =
                self.reconstruct_node_value_with(schema_id, child_id, leaf_value_for)
            {
                child_object.insert(child_key.to_string(), child_value);
            }
        }

        if let Some(leaf_id) = selected_leaf_id {
            if !child_object.is_empty() {
                panic!(
                    "schema id mixes leaf and object for one path at leaf id {:?}",
                    leaf_id
                );
            }
            let leaf_info = self.leaf_info(leaf_id);
            return Some(leaf_value_for(leaf_id, leaf_info));
        }

        if child_object.is_empty() {
            None
        } else {
            Some(JsonValue::Object(child_object))
        }
    }

    fn sorted_children(&self, node_id: NodeId) -> Vec<(&str, NodeId)> {
        let mut children: Vec<(&str, NodeId)> = self.nodes[node_id.0 as usize]
            .children
            .iter()
            .map(|(key, child_id)| (key.as_str(), *child_id))
            .collect();
        children
            .sort_unstable_by(|(child_key_1, _), (child_key_2, _)| child_key_1.cmp(child_key_2));
        children
    }

    fn get_or_create_child(&mut self, parent_id: NodeId, key: &str) -> NodeId {
        let parent_index = parent_id.0 as usize;
        if let Some(child_id) = self.nodes[parent_index].children.get(key) {
            return *child_id;
        }

        let child_id = NodeId(self.nodes.len() as u32);
        self.nodes.push(SchemaNode::default());
        self.nodes[parent_index]
            .children
            .insert(key.to_string(), child_id);
        child_id
    }

    fn intern_leaf(&mut self, node_id: NodeId, key: &str, kind: LeafKind) -> LeafId {
        let node_index = node_id.0 as usize;
        if let Some(existing) = self.nodes[node_index].leaves[kind.index()] {
            return existing;
        }

        let id = LeafId(self.leaves.len() as u32);
        self.nodes[node_index].leaves[kind.index()] = Some(id);
        self.leaves.push(LeafInfo {
            key: key.to_string(),
            kind,
        });
        id
    }

    fn parse_error(err: serde_json::Error) -> SchemaError {
        SchemaError::Parse(io::Error::new(io::ErrorKind::InvalidData, err))
    }
}

struct ObjectSeedWithCallback<'a, F> {
    tree: &'a mut SchemaTree,
    node_id: NodeId,
    out: &'a mut Vec<LeafId>,
    on_leaf: &'a mut F,
}

impl<'de, F> DeserializeSeed<'de> for ObjectSeedWithCallback<'_, F>
where
    F: FnMut(LeafId, &Value),
{
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(ObjectVisitorWithCallback {
            tree: self.tree,
            node_id: self.node_id,
            out: self.out,
            on_leaf: self.on_leaf,
        })
    }
}

struct ObjectVisitorWithCallback<'a, F> {
    tree: &'a mut SchemaTree,
    node_id: NodeId,
    out: &'a mut Vec<LeafId>,
    on_leaf: &'a mut F,
}

impl<'de, F> Visitor<'de> for ObjectVisitorWithCallback<'_, F>
where
    F: FnMut(LeafId, &Value),
{
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON object")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        parse_object_entries_with_callback(
            &mut map,
            self.tree,
            self.node_id,
            self.out,
            self.on_leaf,
        )
    }
}

struct ValueSeedWithCallback<'a, 'k, F> {
    tree: &'a mut SchemaTree,
    node_id: NodeId,
    key: &'k str,
    out: &'a mut Vec<LeafId>,
    on_leaf: &'a mut F,
}

impl<'de, F> DeserializeSeed<'de> for ValueSeedWithCallback<'_, '_, F>
where
    F: FnMut(LeafId, &Value),
{
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(ValueVisitorWithCallback {
            tree: self.tree,
            node_id: self.node_id,
            key: self.key,
            out: self.out,
            on_leaf: self.on_leaf,
        })
    }
}

struct ValueVisitorWithCallback<'a, 'k, F> {
    tree: &'a mut SchemaTree,
    node_id: NodeId,
    key: &'k str,
    out: &'a mut Vec<LeafId>,
    on_leaf: &'a mut F,
}

impl<'de, F> Visitor<'de> for ValueVisitorWithCallback<'_, '_, F>
where
    F: FnMut(LeafId, &Value),
{
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        parse_object_entries_with_callback(
            &mut map,
            self.tree,
            self.node_id,
            self.out,
            self.on_leaf,
        )
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut array_elements = Vec::new();
        while let Some(element) = seq.next_element::<Value<'de>>()? {
            array_elements.push(element);
        }
        let leaf_value = Value::Array(array_elements);
        self.emit_leaf(LeafKind::Array, &leaf_value);
        Ok(())
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let leaf_value = Value::Bool(value);
        self.emit_leaf(LeafKind::Bool, &leaf_value);
        Ok(())
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let leaf_value = Value::from(value);
        self.emit_leaf(LeafKind::Number, &leaf_value);
        Ok(())
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let leaf_value = Value::from(value);
        self.emit_leaf(LeafKind::Number, &leaf_value);
        Ok(())
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let leaf_value = Value::from(value);
        self.emit_leaf(LeafKind::Number, &leaf_value);
        Ok(())
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let leaf_value = Value::Str(Cow::Borrowed(value));
        self.emit_leaf(LeafKind::String, &leaf_value);
        Ok(())
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let leaf_value = Value::Null;
        self.emit_leaf(LeafKind::Null, &leaf_value);
        Ok(())
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let leaf_value = Value::Null;
        self.emit_leaf(LeafKind::Null, &leaf_value);
        Ok(())
    }
}

impl<F> ValueVisitorWithCallback<'_, '_, F>
where
    F: FnMut(LeafId, &Value),
{
    fn emit_leaf(self, kind: LeafKind, leaf_value: &Value) {
        let leaf_id = self.tree.intern_leaf(self.node_id, self.key, kind);
        self.out.push(leaf_id);
        (self.on_leaf)(leaf_id, leaf_value);
    }
}

fn parse_object_entries_with_callback<'de, A, F>(
    map: &mut A,
    tree: &mut SchemaTree,
    node_id: NodeId,
    out: &mut Vec<LeafId>,
    on_leaf: &mut F,
) -> Result<(), A::Error>
where
    A: MapAccess<'de>,
    F: FnMut(LeafId, &Value),
{
    while let Some(key) = map.next_key::<Cow<'de, str>>()? {
        let child_id = tree.get_or_create_child(node_id, key.as_ref());
        map.next_value_seed(ValueSeedWithCallback {
            tree,
            node_id: child_id,
            key: key.as_ref(),
            out,
            on_leaf: &mut *on_leaf,
        })?;
    }
    Ok(())
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
    fn malformed_json_returns_parse_error() {
        let mut tree = SchemaTree::new();
        let err = tree.ingest_json("tru").unwrap_err();
        assert!(matches!(err, SchemaError::Parse(_)));
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
    fn reconstructs_leaf_key() {
        let mut tree = SchemaTree::new();
        let schema_id = tree.ingest_json(r#"{"a": {"b": 1}}"#).unwrap();
        let leaf_infos = schema_id.reconstruct(&tree);
        let leaf_info = leaf_infos[0];

        assert_eq!(leaf_info.key, "b");
    }

    #[test]
    fn reconstructs_json_object_with_placeholders() {
        let mut tree = SchemaTree::new();
        let schema_id = tree
            .ingest_json(
                r#"{"z": 1, "nested": {"text": "x"}, "arr": [1], "flag": true, "nil": null}"#,
            )
            .unwrap();

        let reconstructed_json = schema_id.reconstruct_json(&tree);
        let reconstructed: serde_json::Value = serde_json::from_str(&reconstructed_json).unwrap();
        let expected = serde_json::json!({
            "arr": [],
            "flag": false,
            "nested": { "text": "" },
            "nil": null,
            "z": 0,
        });
        assert_eq!(reconstructed, expected);
    }

    #[test]
    fn reconstructs_json_object_with_custom_leaf_values() {
        let mut tree = SchemaTree::new();
        let mut values_by_leaf_id = FxHashMap::default();
        let original_json =
            r#"{"z": 42, "nested": {"text": "abc"}, "arr": [1, 2], "flag": true, "nil": null}"#;
        let schema_id = tree
            .ingest_json_with(original_json, |leaf_id, value| {
                let reconstructed_value: serde_json::Value =
                    serde_json::from_str(&value.to_string())
                        .expect("leaf JSON value should round-trip");
                values_by_leaf_id.insert(leaf_id, reconstructed_value);
            })
            .unwrap();

        let reconstructed_json = schema_id.reconstruct_json_with(&tree, |leaf_id, _| {
            values_by_leaf_id
                .get(&leaf_id)
                .cloned()
                .expect("value for leaf id should exist")
        });
        let reconstructed: serde_json::Value = serde_json::from_str(&reconstructed_json).unwrap();
        let expected: serde_json::Value = serde_json::from_str(original_json).unwrap();
        assert_eq!(reconstructed, expected);
    }
}

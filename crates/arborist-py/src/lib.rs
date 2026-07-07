use std::cell::RefCell;
use std::path::Path;

use arborist_core::{
    PatchAstNodeResult, PositionEdit, TraceDirection, TraceSymbolGraphResult, VirtualFileSystem,
    execute_tree_query, execute_tree_query_from_path, get_semantic_skeleton,
    get_semantic_skeleton_from_path, patch_ast_node, rebuild_symbol_index,
    refresh_symbol_index_for_file, replay_patch_evidence_against_trace, supported_languages,
    trace_symbol_graph_from_index, validate_patch_commit_with_trace,
    validate_patch_with_trace_context, validate_patch_with_trace_context_from_path,
};
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use serde::de::{self, DeserializeOwned, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::fmt;

#[pyclass(unsendable)]
struct ArboristCore {
    vfs: RefCell<VirtualFileSystem>,
}

#[pymethods]
impl ArboristCore {
    #[new]
    fn new() -> Self {
        Self {
            vfs: RefCell::new(VirtualFileSystem::new()),
        }
    }

    fn supported_languages(&self) -> Vec<String> {
        supported_languages()
            .into_iter()
            .map(str::to_string)
            .collect()
    }

    #[pyo3(signature = (file_path, source=None, depth_limit=2, expand_nodes=None))]
    fn get_semantic_skeleton_json(
        &self,
        file_path: &str,
        source: Option<String>,
        depth_limit: usize,
        expand_nodes: Option<Vec<String>>,
    ) -> PyResult<String> {
        let expand_nodes = expand_nodes.unwrap_or_default();
        let result = match source {
            Some(source) => {
                get_semantic_skeleton(Path::new(file_path), &source, depth_limit, &expand_nodes)
            }
            None => {
                get_semantic_skeleton_from_path(Path::new(file_path), depth_limit, &expand_nodes)
            }
        }
        .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    #[pyo3(signature = (file_path, query, source=None))]
    fn execute_tree_query_json(
        &self,
        file_path: &str,
        query: &str,
        source: Option<String>,
    ) -> PyResult<String> {
        let result = match source {
            Some(source) => execute_tree_query(Path::new(file_path), &source, query),
            None => execute_tree_query_from_path(Path::new(file_path), query),
        }
        .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    #[pyo3(signature = (file_path, semantic_path, new_code, source=None, bypass_reason=None))]
    fn patch_ast_node_json(
        &self,
        file_path: &str,
        semantic_path: &str,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
    ) -> PyResult<String> {
        let result = match source {
            Some(source) => patch_ast_node(
                Path::new(file_path),
                &source,
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
            ),
            None => {
                let mut vfs = self.vfs.borrow_mut();
                let result = vfs
                    .patch_node(
                        Path::new(file_path),
                        semantic_path,
                        new_code,
                        bypass_reason.as_deref(),
                    )
                    .map_err(to_py_error)?;
                if result.applied {
                    vfs.commit_file(Path::new(file_path)).map_err(to_py_error)?;
                }
                Ok(result)
            }
        }
        .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    fn patch_virtual_ast_node_json(
        &self,
        file_path: &str,
        semantic_path: &str,
        new_code: &str,
        bypass_reason: Option<String>,
    ) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .patch_node(
                Path::new(file_path),
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
            )
            .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    #[pyo3(signature = (workspace_root, symbol_path, direction="both", index_db_path=None))]
    fn trace_symbol_graph_json(
        &self,
        workspace_root: &str,
        symbol_path: &str,
        direction: &str,
        index_db_path: Option<String>,
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let result = match index_db_path {
            Some(index_db_path) => {
                trace_symbol_graph_from_index(Path::new(&index_db_path), symbol_path, direction)
            }
            None => self.vfs.borrow_mut().trace_symbol_graph(
                Path::new(workspace_root),
                symbol_path,
                direction,
            ),
        }
        .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    fn replay_patch_evidence_against_trace_json(
        &self,
        patch_json: &str,
        trace_json: &str,
    ) -> PyResult<String> {
        let patch: PatchAstNodeResult = parse_json_arg(patch_json)?;
        let trace: TraceSymbolGraphResult = parse_json_arg(trace_json)?;
        let result = replay_patch_evidence_against_trace(&patch, &trace);
        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    fn validate_patch_commit_with_trace_json(
        &self,
        patch_json: &str,
        trace_json: &str,
    ) -> PyResult<String> {
        let patch: PatchAstNodeResult = parse_json_arg(patch_json)?;
        let trace: TraceSymbolGraphResult = parse_json_arg(trace_json)?;
        let result = validate_patch_commit_with_trace(&patch, &trace);
        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    #[pyo3(signature = (workspace_root, file_path, semantic_path, new_code, source=None, bypass_reason=None, direction="both"))]
    // Keep the Python binding signature aligned with the JSON-RPC parameter surface.
    #[allow(clippy::too_many_arguments)]
    fn validate_patch_with_trace_context_json(
        &self,
        workspace_root: &str,
        file_path: &str,
        semantic_path: &str,
        new_code: &str,
        source: Option<String>,
        bypass_reason: Option<String>,
        direction: &str,
    ) -> PyResult<String> {
        let direction = parse_direction(direction)?;
        let result = match source {
            Some(source) => validate_patch_with_trace_context(
                Path::new(workspace_root),
                Path::new(file_path),
                &source,
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
                direction,
            ),
            None => validate_patch_with_trace_context_from_path(
                Path::new(workspace_root),
                Path::new(file_path),
                semantic_path,
                new_code,
                bypass_reason.as_deref(),
                direction,
            ),
        }
        .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    fn rebuild_symbol_index_json(&self, workspace_root: &str, db_path: &str) -> PyResult<String> {
        let result = rebuild_symbol_index(Path::new(workspace_root), Path::new(db_path))
            .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    fn refresh_symbol_index_for_file_json(
        &self,
        workspace_root: &str,
        db_path: &str,
        file_path: &str,
    ) -> PyResult<String> {
        let result = refresh_symbol_index_for_file(
            Path::new(workspace_root),
            Path::new(db_path),
            Path::new(file_path),
        )
        .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    fn register_symbol_index_json(&self, workspace_root: &str, db_path: &str) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .register_symbol_index(Path::new(workspace_root), Path::new(db_path))
            .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    fn unregister_symbol_index_json(&self, workspace_root: &str) -> PyResult<bool> {
        self.vfs
            .borrow_mut()
            .unregister_symbol_index(Path::new(workspace_root))
            .map_err(to_py_error)
    }

    fn list_symbol_indexes_json(&self) -> PyResult<String> {
        let result = self.vfs.borrow().registered_symbol_indexes();
        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    fn open_virtual_file_json(&self, file_path: &str, source: Option<String>) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .open_file(Path::new(file_path), source.as_deref())
            .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    fn read_virtual_file_json(&self, file_path: &str) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .read_file(Path::new(file_path))
            .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    fn list_virtual_files_json(&self, dirty_only: bool) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .virtual_file_statuses(dirty_only)
            .map_err(to_py_error)?;
        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    fn apply_buffer_edit_json(
        &self,
        file_path: &str,
        start_byte: usize,
        old_end_byte: usize,
        new_text: &str,
    ) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .apply_edit(Path::new(file_path), start_byte, old_end_byte, new_text)
            .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    fn apply_position_edits_json(&self, file_path: &str, edits_json: &str) -> PyResult<String> {
        let edits: Vec<PositionEdit> = parse_json_arg(edits_json)?;
        let result = self
            .vfs
            .borrow_mut()
            .apply_position_edits(Path::new(file_path), &edits)
            .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    fn commit_virtual_file_json(&self, file_path: &str) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .commit_file(Path::new(file_path))
            .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    fn discard_virtual_file_json(&self, file_path: &str) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .discard_file(Path::new(file_path))
            .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
    }

    #[pyo3(signature = (file_path, persist=false))]
    fn close_virtual_file_json(&self, file_path: &str, persist: bool) -> PyResult<String> {
        let result = self
            .vfs
            .borrow_mut()
            .close_file(Path::new(file_path), persist)
            .map_err(to_py_error)?;

        serde_json::to_string(&result).map_err(to_runtime_error)
    }
}

fn to_py_error(error: anyhow::Error) -> PyErr {
    PyValueError::new_err(error.to_string())
}

fn to_runtime_error(error: serde_json::Error) -> PyErr {
    PyRuntimeError::new_err(error.to_string())
}

fn parse_json_arg<T: DeserializeOwned>(json: &str) -> PyResult<T> {
    let checked = serde_json::from_str::<DuplicateCheckedJson>(json)
        .map_err(|error| PyValueError::new_err(error.to_string()))?;
    serde_json::from_value(checked.0).map_err(|error| PyValueError::new_err(error.to_string()))
}

struct DuplicateCheckedJson(serde_json::Value);

impl<'de> Deserialize<'de> for DuplicateCheckedJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(DuplicateCheckedJsonVisitor)
    }
}

struct DuplicateCheckedJsonVisitor;

impl<'de> Visitor<'de> for DuplicateCheckedJsonVisitor {
    type Value = DuplicateCheckedJson;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value without duplicate object keys")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(DuplicateCheckedJson(serde_json::Value::Bool(value)))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(DuplicateCheckedJson(serde_json::Value::Number(
            serde_json::Number::from(value),
        )))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(DuplicateCheckedJson(serde_json::Value::Number(
            serde_json::Number::from(value),
        )))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let number =
            serde_json::Number::from_f64(value).ok_or_else(|| E::custom("invalid JSON number"))?;
        Ok(DuplicateCheckedJson(serde_json::Value::Number(number)))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(DuplicateCheckedJson(serde_json::Value::String(
            value.to_string(),
        )))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(DuplicateCheckedJson(serde_json::Value::String(value)))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(DuplicateCheckedJson(serde_json::Value::Null))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(DuplicateCheckedJson(serde_json::Value::Null))
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        DuplicateCheckedJson::deserialize(deserializer)
    }

    fn visit_seq<A>(self, mut access: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::with_capacity(access.size_hint().unwrap_or(0));
        while let Some(value) = access.next_element::<DuplicateCheckedJson>()? {
            values.push(value.0);
        }
        Ok(DuplicateCheckedJson(serde_json::Value::Array(values)))
    }

    fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = serde_json::Map::new();
        while let Some(key) = access.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(de::Error::custom(format!(
                    "duplicate JSON object key `{key}`"
                )));
            }
            let value = access.next_value::<DuplicateCheckedJson>()?;
            values.insert(key, value.0);
        }
        Ok(DuplicateCheckedJson(serde_json::Value::Object(values)))
    }
}

fn parse_direction(direction: &str) -> PyResult<TraceDirection> {
    match direction {
        "callers" => Ok(TraceDirection::Callers),
        "callees" => Ok(TraceDirection::Callees),
        "both" => Ok(TraceDirection::Both),
        other => Err(PyValueError::new_err(format!(
            "invalid direction `{other}`, expected callers|callees|both"
        ))),
    }
}

#[pymodule]
fn _arborist_core(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<ArboristCore>()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{PositionEdit, TraceSymbolGraphResult, parse_json_arg};
    use std::sync::Once;

    fn prepare_python() {
        static PREPARE: Once = Once::new();
        PREPARE.call_once(pyo3::prepare_freethreaded_python);
    }

    #[test]
    fn parse_json_arg_rejects_duplicate_top_level_keys() {
        prepare_python();

        let error = parse_json_arg::<PositionEdit>(
            r#"{"start":{"row":0,"column":0},"end":{"row":0,"column":1},"new_text":"x","new_text":"y"}"#,
        )
        .expect_err("duplicate top-level keys should be rejected");

        assert!(
            error
                .to_string()
                .contains("duplicate JSON object key `new_text`")
        );
    }

    #[test]
    fn parse_json_arg_rejects_duplicate_nested_keys() {
        prepare_python();

        let error = parse_json_arg::<Vec<PositionEdit>>(
            r#"[{"start":{"row":0,"column":0,"row":1},"end":{"row":0,"column":1},"new_text":"x"}]"#,
        )
        .expect_err("duplicate nested keys should be rejected");

        assert!(
            error
                .to_string()
                .contains("duplicate JSON object key `row`")
        );
    }

    #[test]
    fn parse_json_arg_accepts_valid_payloads() {
        prepare_python();

        let edits = parse_json_arg::<Vec<PositionEdit>>(
            r#"[{"start":{"row":0,"column":0},"end":{"row":0,"column":1},"new_text":"x"}]"#,
        )
        .expect("valid edit payload should parse");

        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].new_text, "x");
    }

    #[test]
    fn parse_json_arg_rejects_missing_nested_trace_fields() {
        prepare_python();

        let error = parse_json_arg::<TraceSymbolGraphResult>(
            r#"{
                "symbol":{"symbol_id":"top_level"},
                "callers":[],
                "callees":[],
                "evidence_keys":{
                    "symbol":"top_level|sample.py|function_definition|trace_root|0..10|",
                    "callers":[],
                    "callees":[]
                },
                "indexed_files":1
            }"#,
        )
        .expect_err("trace payloads should reject missing nested symbol fields");

        assert!(error.to_string().contains("missing field"));
    }
}

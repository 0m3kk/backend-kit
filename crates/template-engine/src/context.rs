use serde::Serialize;
use serde_json::{Map, Value};

use crate::errors::TemplateError;

/// Fluent data context container passed to template rendering engines.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct TemplateContext {
    data: Map<String, Value>,
}

impl TemplateContext {
    /// Creates a new, empty `TemplateContext`.
    pub fn new() -> Self {
        Self { data: Map::new() }
    }

    /// Inserts a key-value pair into the template context.
    pub fn insert<T: Serialize>(
        mut self,
        key: impl Into<String>,
        value: &T,
    ) -> Result<Self, TemplateError> {
        let json_val = serde_json::to_value(value).map_err(|e| {
            TemplateError::SerializationError(format!("Failed to serialize key: {e}"))
        })?;
        self.data.insert(key.into(), json_val);
        Ok(self)
    }

    /// Extends the context with fields from a serializable struct or map.
    pub fn extend_from<T: Serialize>(mut self, value: &T) -> Result<Self, TemplateError> {
        let json_val = serde_json::to_value(value).map_err(|e| {
            TemplateError::SerializationError(format!("Failed to serialize struct context: {e}"))
        })?;

        if let Value::Object(map) = json_val {
            for (k, v) in map {
                self.data.insert(k, v);
            }
            Ok(self)
        } else {
            Err(TemplateError::SerializationError(
                "Context extension source must serialize into a JSON Object".to_string(),
            ))
        }
    }

    /// Converts the context into a `serde_json::Value`.
    pub fn into_value(self) -> Value {
        Value::Object(self.data)
    }

    /// Returns a reference to the internal map of context data.
    pub fn as_map(&self) -> &Map<String, Value> {
        &self.data
    }
}

impl From<Map<String, Value>> for TemplateContext {
    fn from(data: Map<String, Value>) -> Self {
        Self { data }
    }
}

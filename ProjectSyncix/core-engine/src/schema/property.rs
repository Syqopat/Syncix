use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PropertyType {
    String,
    Number,
    Boolean,
    Vector2,
    Vector3,
    Color3,
    CFrame,
    Enum(Vec<String>), // enum options
    UDim,
    UDim2,
    Reference, // instance reference
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRules {
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub max_length: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertySchema {
    pub name: String,
    pub property_type: PropertyType,
    pub default_value: serde_json::Value,
    pub read_only: bool,
    pub validation: Option<ValidationRules>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassSchema {
    pub class_name: String,
    pub super_class: Option<String>,
    pub properties: HashMap<String, PropertySchema>,
}

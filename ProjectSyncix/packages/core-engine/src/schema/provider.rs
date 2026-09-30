use super::property::{ClassSchema, PropertySchema, PropertyType, ValidationRules};
use serde_json::json;
use std::collections::HashMap;

pub struct PropertySchemaProvider {
    classes: HashMap<String, ClassSchema>,
}

impl PropertySchemaProvider {
    pub fn new() -> Self {
        let mut provider = Self {
            classes: HashMap::new(),
        };
        provider.register_core_classes();
        provider
    }

    fn register_core_classes(&mut self) {
        // Base Instance
        let mut instance_props = HashMap::new();
        instance_props.insert(
            "Name".to_string(),
            PropertySchema {
                name: "Name".to_string(),
                property_type: PropertyType::String,
                default_value: json!("Instance"),
                read_only: false,
                validation: Some(ValidationRules {
                    min: None,
                    max: None,
                    max_length: Some(100),
                }),
            },
        );
        instance_props.insert(
            "Parent".to_string(),
            PropertySchema {
                name: "Parent".to_string(),
                property_type: PropertyType::Reference,
                default_value: json!(null),
                read_only: false,
                validation: None,
            },
        );

        self.classes.insert(
            "Instance".to_string(),
            ClassSchema {
                class_name: "Instance".to_string(),
                super_class: None,
                properties: instance_props,
            },
        );

        // Part
        let mut part_props = HashMap::new();
        part_props.insert(
            "Position".to_string(),
            PropertySchema {
                name: "Position".to_string(),
                property_type: PropertyType::Vector3,
                default_value: json!({"x": 0.0, "y": 0.0, "z": 0.0}),
                read_only: false,
                validation: None,
            },
        );
        part_props.insert(
            "Size".to_string(),
            PropertySchema {
                name: "Size".to_string(),
                property_type: PropertyType::Vector3,
                default_value: json!({"x": 4.0, "y": 1.0, "z": 2.0}),
                read_only: false,
                validation: None,
            },
        );
        part_props.insert(
            "Color".to_string(),
            PropertySchema {
                name: "Color".to_string(),
                property_type: PropertyType::Color3,
                default_value: json!({"r": 163, "g": 162, "b": 165}),
                read_only: false,
                validation: None,
            },
        );
        part_props.insert(
            "Anchored".to_string(),
            PropertySchema {
                name: "Anchored".to_string(),
                property_type: PropertyType::Boolean,
                default_value: json!(false),
                read_only: false,
                validation: None,
            },
        );
        part_props.insert(
            "Material".to_string(),
            PropertySchema {
                name: "Material".to_string(),
                property_type: PropertyType::Enum(vec![
                    "Plastic".to_string(),
                    "Wood".to_string(),
                    "Neon".to_string(),
                ]),
                default_value: json!("Plastic"),
                read_only: false,
                validation: None,
            },
        );

        self.classes.insert(
            "Part".to_string(),
            ClassSchema {
                class_name: "Part".to_string(),
                super_class: Some("Instance".to_string()),
                properties: part_props,
            },
        );
    }

    pub fn get_class_schema(&self, class_name: &str) -> Option<&ClassSchema> {
        self.classes.get(class_name)
    }

    pub fn get_all_schemas(&self) -> &HashMap<String, ClassSchema> {
        &self.classes
    }
}

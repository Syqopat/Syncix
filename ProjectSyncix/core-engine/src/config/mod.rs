use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Syncix Projesi Manifest Yapısı (syncix.toml)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectManifest {
    pub project: ProjectInfo,
    pub build: BuildSettings,
    pub features: FeatureFlags,
    pub plugins: PluginSettings,
    #[serde(default)]
    pub diagnostics: DiagnosticsSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub name: String,
    pub version: String,
    pub schema_version: u32,
    pub profile: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildSettings {
    pub workspace_dir: String,
    pub auto_export: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlags {
    pub enable_ai: bool,
    pub enable_collaboration: bool,
    pub enable_experimental_serializers: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSettings {
    pub allow_file_system: bool,
    pub allow_network: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticsSettings {
    pub enabled: bool,
    pub telemetry_level: String, // "None", "Error", "Full"
    pub log_retention_days: u32,
    pub max_log_size_mb: u32,
    pub metrics_interval_secs: u32,
}

impl Default for DiagnosticsSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            telemetry_level: "Error".to_string(),
            log_retention_days: 7,
            max_log_size_mb: 50,
            metrics_interval_secs: 60,
        }
    }
}

impl Default for ProjectManifest {
    fn default() -> Self {
        Self {
            project: ProjectInfo {
                name: "SyncixProject".to_string(),
                version: "1.0.0".to_string(),
                schema_version: 2, // Upgraded to v2 for Sprint 3
                profile: Some("Development".to_string()),
            },
            build: BuildSettings {
                workspace_dir: "src".to_string(),
                auto_export: false,
            },
            features: FeatureFlags {
                enable_ai: false,
                enable_collaboration: false,
                enable_experimental_serializers: false,
            },
            plugins: PluginSettings {
                allow_file_system: false,
                allow_network: false,
            },
            diagnostics: DiagnosticsSettings::default(),
        }
    }
}

pub struct ConfigManager {
    manifest: ProjectManifest,
}

impl ConfigManager {
    pub fn new() -> Self {
        Self {
            manifest: ProjectManifest::default(),
        }
    }

    /// syncix.toml dosyasını diskten okur ve valide eder
    pub fn load_from_file(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Err("syncix.toml bulunamadı.".to_string());
        }

        let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
        let mut manifest: ProjectManifest = toml::from_str(&content).map_err(|e| e.to_string())?;

        // Migration from Schema V1 to V2
        if manifest.project.schema_version < 2 {
            println!(
                "[Syncix Config] Migrating syncix.toml to Schema Version 2 (Adding Diagnostics)"
            );
            manifest.project.schema_version = 2;
            manifest.diagnostics = DiagnosticsSettings::default();
            // Automatically save the migrated config back to file
            let migrated_toml = toml::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
            let _ = fs::write(path, migrated_toml);
        }

        // Validation Rules
        if manifest.diagnostics.max_log_size_mb > 1024 {
            manifest.diagnostics.max_log_size_mb = 1024; // Cap at 1GB
        }
        if manifest.diagnostics.telemetry_level != "None"
            && manifest.diagnostics.telemetry_level != "Error"
            && manifest.diagnostics.telemetry_level != "Full"
        {
            manifest.diagnostics.telemetry_level = "Error".to_string(); // Fallback
        }

        Ok(Self { manifest })
    }

    pub fn get_manifest(&self) -> &ProjectManifest {
        &self.manifest
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_manifest() {
        let config = ConfigManager::new();
        assert_eq!(config.get_manifest().project.schema_version, 2);
        assert!(!config.get_manifest().features.enable_ai);
    }

    #[test]
    fn test_toml_parsing() {
        let toml_str = r#"
            [project]
            name = "TestProject"
            version = "2.0.0"
            schema_version = 2
            profile = "Production"

            [build]
            workspace_dir = "out"
            auto_export = true

            [features]
            enable_ai = true
            enable_collaboration = false
            enable_experimental_serializers = true

            [plugins]
            allow_file_system = true
            allow_network = false
        "#;

        let manifest: ProjectManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(manifest.project.name, "TestProject");
        assert_eq!(manifest.project.schema_version, 2);
        assert!(manifest.features.enable_ai);
    }
}

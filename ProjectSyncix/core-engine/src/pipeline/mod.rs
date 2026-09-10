use crate::model::InstanceNode;
use serde_json::Value;

/// Dışa Aktarma Katmanı (Export Pipeline)
/// Mevcut DataModel ağacını farklı formatlara çevirir.
pub struct ExportPipeline;

impl ExportPipeline {
    /// DataModel'i devasa bir JSON dökümü olarak dışa aktarır
    pub fn export_to_json(root: &InstanceNode) -> String {
        serde_json::to_string_pretty(root).unwrap()
    }

    /// İleride eklenecek: rbxlx (XML) formatında dışa aktarır
    pub fn export_to_rbxlx(_root: &InstanceNode) -> String {
        unimplemented!("RBXLX Export henüz desteklenmiyor.")
    }
}

/// İçe Aktarma Katmanı (Import Pipeline)
/// Farklı formatlardan received verileri InstanceNode ağacına çevirir.
pub struct ImportPipeline;

impl ImportPipeline {
    /// JSON dump dosyasından DataModel'i yükler
    pub fn import_from_json(json_str: &str) -> Result<InstanceNode, String> {
        let root: InstanceNode = serde_json::from_str(json_str).map_err(|e| e.to_string())?;
        Ok(root)
    }
}

/// Veritabanı (Şema) Göç Katmanı (Migration Pipeline)
/// Versiyon değişikliklerinde previous_text formatları fresh formata çevirir.
pub struct MigrationPipeline;

impl MigrationPipeline {
    /// v1 şemasından v2 şemasına geçiş yapar.
    /// Örn: "Color" property'si Color3 nesnesinden String'e döndüyse burada çevirilir.
    pub fn migrate_v1_to_v2(old_json: &str) -> Result<String, String> {
        let mut v: Value = serde_json::from_str(old_json).map_err(|e| e.to_string())?;

        // Örnek Migration: "class_name" -> "ClassName" olarak büyük harfe çevriliyorsa:
        if let Some(class_name) = v.get("class_name").cloned() {
            if let Some(obj) = v.as_object_mut() {
                obj.insert("ClassName".to_string(), class_name);
                obj.remove("class_name");
            }
        }

        serde_json::to_string(&v).map_err(|e| e.to_string())
    }
}

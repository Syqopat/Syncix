use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

/// Referans Çözümleyici (ReferenceResolver)
/// Nesneler arasındaki ObjectValue, Motor, HingeConstraint gibi referansları yönetir.
/// Eğer referans edilen nesne (UUID) henüz Workspace'te yoksa "Lazy Resolution" için beklemeye alır.
pub struct ReferenceResolver {
    /// UUID -> Bu UUID'yi pending_item nesneler ve property isimleri
    /// Örn: "Part_B" UUID'si yaratıldığında, "ObjectValue_A" nın "Value" propertysine atanacak.
    pending_references: RwLock<HashMap<String, Vec<PendingRef>>>,

    /// Şu ana kadar çözülmüş referansların bir haritası (Hard References)
    resolved_references: RwLock<HashSet<String>>,
}

pub struct PendingRef {
    pub source_uuid: String,
    pub property_name: String,
}

impl ReferenceResolver {
    pub fn new() -> Self {
        Self {
            pending_references: RwLock::new(HashMap::new()),
            resolved_references: RwLock::new(HashSet::new()),
        }
    }

    /// Bir referans talebini sıraya alır.
    pub fn enqueue_reference(&self, target_uuid: &str, source_uuid: &str, property_name: &str) {
        let mut pending = self.pending_references.write().unwrap();
        let entry = pending.entry(target_uuid.to_string()).or_default();
        entry.push(PendingRef {
            source_uuid: source_uuid.to_string(),
            property_name: property_name.to_string(),
        });
    }

    /// Yeni bir UUID sisteme dahil olduğunda (Instance yaratıldığında) çağrılır.
    /// Eğer bu UUID'yi pending_item referanslar varsa onları çözümler (Resolve).
    pub fn notify_uuid_created(&self, new_uuid: &str) -> Vec<PendingRef> {
        self.resolved_references
            .write()
            .unwrap()
            .insert(new_uuid.to_string());

        let mut pending = self.pending_references.write().unwrap();
        if let Some(waiting_list) = pending.remove(new_uuid) {
            return waiting_list; // Bunlar Studio tarafında Dispatcher'a yönlendirilip bağlanacak
        }

        Vec::new()
    }
}

/// Asset Kayıt Sistemi (AssetRegistry)
/// İleride eklenecek Mesh, Texture, Sound gibi yerel dosyaları veya rbxassetid:// linklerini tutar.
pub struct AssetRegistry {
    /// Yerel file_path yolu -> rbxassetid veya syncix:// URL'si
    assets: RwLock<HashMap<String, String>>,
}

impl AssetRegistry {
    pub fn new() -> Self {
        Self {
            assets: RwLock::new(HashMap::new()),
        }
    }

    pub fn register_asset(&self, local_path: &str, asset_id: &str) {
        self.assets
            .write()
            .unwrap()
            .insert(local_path.to_string(), asset_id.to_string());
    }

    pub fn get_asset_id(&self, local_path: &str) -> Option<String> {
        self.assets.read().unwrap().get(local_path).cloned()
    }
}

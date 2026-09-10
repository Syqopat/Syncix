pub mod part;

use crate::model::InstanceNode;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Tüm Serializer'ların uygulaması gereken arayüz.
pub trait Serializer: Send + Sync {
    fn serialize(&self, instance: &InstanceNode) -> Result<String, String>;
    fn deserialize(&self, data: &str) -> Result<InstanceNode, String>;
    fn get_class_name(&self) -> &'static str;
}

/// Sınıf ayrımı yapmayan genel serializer.
/// Kayıtlı özel bir serializer'ı olmayan HER sınıf (Folder, Script, Model, GUI, service_list...)
/// bunun üzerinden diske yazılır. Böylece Studio'daki her obje bir file_path olarak görünür.
pub struct GenericSerializer;

impl Serializer for GenericSerializer {
    fn serialize(&self, instance: &InstanceNode) -> Result<String, String> {
        serde_json::to_string_pretty(instance).map_err(|e| e.to_string())
    }

    fn deserialize(&self, data: &str) -> Result<InstanceNode, String> {
        serde_json::from_str(data).map_err(|e| e.to_string())
    }

    fn get_class_name(&self) -> &'static str {
        "Instance"
    }
}

/// Serializer'ları barındıran merkezi kayıt defteri (Registry & Factory).
/// Performans ve Genişletilebilirlik: Çekirdek motor hangi instance türünü işlediğini bilmez.
/// Gelen verinin `class_name`'ine bakar ve bu defterden ilgili Serializer'ı çeker.
/// 50 fresh nesne eklense bile `main.rs` veya `model.rs` değişmez (Open/Closed Principle).
pub struct SerializerRegistry {
    serializers: HashMap<String, Box<dyn Serializer>>,
    /// Kayıtlı özel serializer'ı olmayan sınıflar için genel yedek.
    fallback: Box<dyn Serializer>,
}

impl SerializerRegistry {
    pub fn new() -> Self {
        Self {
            serializers: HashMap::new(),
            fallback: Box::new(GenericSerializer),
        }
    }

    /// Yeni bir serializer kaydeder.
    pub fn register(&mut self, serializer: Box<dyn Serializer>) {
        let class_name = serializer.get_class_name().to_string();
        self.serializers.insert(class_name, serializer);
    }

    /// İlgili sınıf için serializer döndürür.
    /// Özel bir serializer yoksa genel serializer'a düşer; yani HER sınıf yazılabilir.
    pub fn get(&self, class_name: &str) -> Option<&dyn Serializer> {
        Some(
            self.serializers
                .get(class_name)
                .map(|b| b.as_ref())
                .unwrap_or_else(|| self.fallback.as_ref()),
        )
    }
}

/// Tüm sistemde paylaşılacak olan thread-safe registry.
pub type SharedRegistry = Arc<RwLock<SerializerRegistry>>;

/// İçerisinde varsayılan (Part vb.) serializer'ların yüklü olduğu kayıt defterini oluşturur.
pub fn create_default_registry() -> SharedRegistry {
    let mut registry = SerializerRegistry::new();

    // Tüm current_value serializer'lar burada kaydedilir.
    registry.register(Box::new(part::PartSerializer));

    Arc::new(RwLock::new(registry))
}

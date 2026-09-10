use crate::model::InstanceNode;
use crate::serializers::SharedRegistry;
use crate::vfs::FileSystemProvider;
use std::path::Path;
use std::sync::Arc;

/// TreeBuilder
/// VFS üzerinden klasör yapısını okuyarak Recursive olarak Roblox Ağacını (Tree) inşa eder.
/// Kurallar:
/// 1. `init.class_name.json` varsa (Örn: init.model.json), klasör o sınıfa dönüşür.
/// 2. İçindeki diğer `.json` dosyaları o kök sınıfın çocukları olur.
pub struct TreeBuilder {
    vfs: Arc<dyn FileSystemProvider>,
    registry: SharedRegistry,
}

impl TreeBuilder {
    pub fn new(vfs: Arc<dyn FileSystemProvider>, registry: SharedRegistry) -> Self {
        Self { vfs, registry }
    }

    /// Bir klasörden yola çıkarak DataModel ağacını oluşturur
    pub async fn build_from_dir(&self, root_path: &Path) -> Result<Option<InstanceNode>, String> {
        if !self.vfs.is_dir(root_path) {
            return Err("Root path must be a directory".into());
        }

        Box::pin(self.process_directory(root_path)).await
    }

    async fn process_directory(&self, dir_path: &Path) -> Result<Option<InstanceNode>, String> {
        let entries = self.vfs.read_dir(dir_path).map_err(|e| e.to_string())?;

        // 1. Önce bu klasörün bir "Kök Dosyası" (init.*.json) exists_flag mı bul
        let mut root_node = None;

        for path in &entries {
            if self.vfs.is_file(path) {
                if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                    if filename.starts_with("init.") && filename.ends_with(".json") {
                        // init.model.json gibi bir file_path was_found
                        let data = self.vfs.read_to_string(path).map_err(|e| e.to_string())?;

                        // Sınıf adını dosyadan çıkar (init.model.json -> Model)
                        // Dosya deserialize edilerek gerçek değerleri elde edilecek
                        // Şimdilik deserialization işlemini Registry'den geçireceğiz
                        // Not: JSON dosyasının içinde zaten class_name exists_flag. Herhangi bir serializer onu açabilir
                        // Ancak GenericSerializer kullanmak için Registry'den Generic serializer'ı çekmeliyiz.

                        let node = self.deserialize_file(&data).await?;
                        root_node = Some(node);
                        break; // Sadece bir init dosyası olabilir
                    }
                }
            }
        }

        // Eğer init dosyası yoksa, bu klasör doğrudan bir Roblox nesnesi sayılmaz.
        // Ama test amaçlı veya Rojo uyumluluğu için varsayılan olarak Folder sayılabilir mi?
        // Kullanıcı kararı: "Normal klasör yalnızca fiziksel klasör olsun"
        // Dolayısıyla root_node yoksa `None` döneceğiz (Alt dosyaları DataModel'e bağlanmaz).
        let mut node = match root_node {
            Some(n) => n,
            None => return Ok(None), // Bu klasör bir Roblox objesi temsil etmiyor
        };

        // 2. Çocukları işle
        for path in &entries {
            if self.vfs.is_dir(path) {
                // Alt klasörler özyinelemeli çağrılır
                if let Ok(Some(child_node)) = Box::pin(self.process_directory(path)).await {
                    node.add_child(child_node.syncix_id);
                }
            } else if self.vfs.is_file(path) {
                if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                    // init dosyası dışındaki diğer .json dosyaları çocuk objelerdir
                    if filename.ends_with(".json") && !filename.starts_with("init.") {
                        let data = self.vfs.read_to_string(path).map_err(|e| e.to_string())?;
                        if let Ok(child_node) = self.deserialize_file(&data).await {
                            node.add_child(child_node.syncix_id);
                        }
                    }
                }
            }
        }

        Ok(Some(node))
    }

    async fn deserialize_file(&self, json_data: &str) -> Result<InstanceNode, String> {
        // Geçici olarak JSON'ı parse edip class_name'i bul
        let v: serde_json::Value = serde_json::from_str(json_data).map_err(|e| e.to_string())?;
        let class_name = v["class_name"]
            .as_str()
            .ok_or("Missing class_name in JSON")?;

        let registry = self.registry.read().await;
        let serializer = registry
            .get(class_name)
            .ok_or(format!("No serializer found for {}", class_name))?;

        serializer.deserialize(json_data)
    }
}

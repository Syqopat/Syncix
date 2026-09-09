use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum ModelError {
    #[error("Instance not found: {0}")]
    InstanceNotFound(Uuid),
    #[error("Stale version (conflict): current {current}, incoming {incoming}")]
    VersionConflict { current: i64, incoming: i64 },
    #[error("Invalid parent: {0}")]
    InvalidParent(Uuid),
}

/// Syncix'in bağımsız, kendi iç veri modeli.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct InstanceNode {
    pub class_name: String,
    pub name: String,
    pub syncix_id: Uuid,
    /// Model versiyonlaması (Migration için)
    pub schema_version: u32,
    /// Çakışma yönetimi (Conflict Resolution) için. Unix timestamp milisaniye.
    pub last_updated: i64,
    pub properties: BTreeMap<String, PropertyValue>,
    pub children: Vec<Uuid>,
    pub parent: Option<Uuid>,
    /// Script sınıfları için kaynak kodu (Script/LocalScript/ModuleScript).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Roblox Attribute'ları (SetAttribute ile eklenen özel değerler).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attributes: BTreeMap<String, PropertyValue>,
    /// CollectionService etiketleri.
    ///
    /// Property değil ayrı bir kanal: Roblox'ta etiketler instance üzerinde bir
    /// alan olarak durmuyor, CollectionService'te tutuluyor. Etiketle çalışan
    /// bir oyunda mantığın önemli bir kısmı buradan geçtiği için, taşınmadığı
    /// sürece editör oyunun yarısını göremiyordu.
    ///
    /// Sıralı ve tekrarsız tutulmalı; küme yerine Vec kullanılıp yazarken
    /// sıralanıyor ki iki taraf aynı listeyi aynı sırada görsün.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

impl InstanceNode {
    pub fn new(class_name: &str, name: &str) -> Self {
        Self {
            class_name: class_name.to_string(),
            name: name.to_string(),
            syncix_id: Uuid::new_v4(),
            schema_version: 1,
            last_updated: Utc::now().timestamp_millis(),
            properties: BTreeMap::new(),
            children: Vec::new(),
            parent: None,
            source: None,
            attributes: BTreeMap::new(),
            tags: Vec::new(),
        }
    }

    pub fn add_child(&mut self, child_id: Uuid) {
        if !self.children.contains(&child_id) {
            self.children.push(child_id);
        }
    }
}

impl Default for InstanceNode {
    fn default() -> Self {
        Self {
            class_name: String::new(),
            name: String::new(),
            syncix_id: Uuid::nil(),
            schema_version: 1,
            last_updated: 0,
            properties: BTreeMap::new(),
            children: Vec::new(),
            parent: None,
            source: None,
            attributes: BTreeMap::new(),
            tags: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PropertyValue {
    String(String),
    Number(f64),
    Boolean(bool),
    Vector3 { x: f32, y: f32, z: f32 },
    Color3 { r: f32, g: f32, b: f32 },
    /// GUI için: UDim2 = (X: scale/offset, Y: scale/offset)
    UDim2 {
        xs: f32,
        xo: f32,
        ys: f32,
        yo: f32,
    },
    Vector2 {
        x: f32,
        y: f32,
    },
    UDim {
        scale: f32,
        offset: f32,
    },
    /// Konum + 3x3 dönme matrisi. Orientation yeterli değil: bir parçanın
    /// gerçek yönelimi Euler açılarıyla tam ifade edilemiyor.
    CFrame {
        pos: [f32; 3],
        rot: [f32; 9],
    },
    NumberRange {
        min: f32,
        max: f32,
    },
    /// Başka bir instance'a referans (ObjectValue.Value, Motor6D.Part0,
    /// Model.PrimaryPart gibi). Değer hedefin UUID'sidir; boş dize = nil.
    ///
    /// Bunun ayrı bir tip olması şart: metin olarak taşınsa iki taraf onu
    /// düz bir metin sanıp instance'a çeviremezdi.
    Ref(String),
    /// Roblox'un adlandırılmış renk paleti ("Really red", "Deep orange").
    ///
    /// Ref ile aynı gerekçe: bir süre düz String olarak taşındı ve Studio
    /// tarafında `part.BrickColor = "Really red"` ataması sessizce başarısız
    /// oldu — metin, BrickColor'a örtük olarak dönüşmüyor. Tip kararı değere
    /// göre verildiği için değerin kendisi tipini taşımak zorunda.
    BrickColor(String),
    /// Asset referansi: "rbxassetid://123". MeshId, SoundId, Image, Texture.
    ///
    /// String'den ayri tutuluyor cunku Roblox'un yeni Content tipi duz metin
    /// atamasini kabul etmiyor; hangi yolla yazilacagini bilmek gerekiyor.
    Content(String),
    /// Renk egrisi: ParticleEmitter.Color, UIGradient.Color, Beam.Color.
    /// Her nokta (zaman, renk); Roblox ara degerleri kendi hesapliyor.
    ColorSequence(Vec<ColorKeypoint>),
    /// Sayi egrisi: seffaflik, boyut, UIGradient.Transparency.
    /// envelope Roblox'un rastgelelik payi; sifir birakilamaz, bilgi tasiyor.
    NumberSequence(Vec<NumberKeypoint>),
    /// 9-slice UI icin dikdortgen (ImageLabel.SliceCenter).
    Rect {
        min: [f32; 2],
        max: [f32; 2],
    },
    /// Yazi tipi. Enum degil bilesik bir yapi: aile + kalinlik + stil.
    Font {
        family: String,
        weight: String,
        style: String,
    },
    /// Ozel fizik: yogunluk, surtunme, esneklik ve agirliklari.
    PhysicalProperties {
        density: f32,
        friction: f32,
        elasticity: f32,
        friction_weight: f32,
        elasticity_weight: f32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ColorKeypoint {
    pub t: f32,
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NumberKeypoint {
    pub t: f32,
    pub v: f32,
    pub envelope: f32,
}

/// İki nesne arasındaki farkları tutan yama (Patch) yapısı.
/// Ağa tüm objeyi değil, sadece bu Patch'i göndereceğiz.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstancePatch {
    pub syncix_id: Uuid,
    pub changed_properties: BTreeMap<String, PropertyValue>,
}

impl InstanceNode {
    /// Eski bir node ile bu node'u kıyaslar ve sadece değişen özellikleri Patch olarak döndürür.
    /// Performans Gerekçesi: Ağa megabaytlarca veri basmak yerine sadece değişen Color3 veya Position basılır.
    pub fn diff(&self, old_node: &InstanceNode) -> Option<InstancePatch> {
        if self.syncix_id != old_node.syncix_id {
            return None; // Farklı nesneler kıyaslanamaz
        }

        let mut changed_properties = BTreeMap::new();

        // İsim değişti mi? (Name özel bir property gibi muamele görür)
        if self.name != old_node.name {
            changed_properties.insert("Name".to_string(), PropertyValue::String(self.name.clone()));
        }

        // Parent değişti mi?
        if self.parent != old_node.parent {
            if let Some(parent_uuid) = self.parent {
                changed_properties.insert("Parent".to_string(), PropertyValue::String(parent_uuid.to_string()));
            } else {
                changed_properties.insert("Parent".to_string(), PropertyValue::String("Workspace".to_string()));
            }
        }

        for (key, new_val) in &self.properties {
            if let Some(old_val) = old_node.properties.get(key) {
                if new_val != old_val {
                    changed_properties.insert(key.clone(), new_val.clone());
                }
            } else {
                // Yeni özellik eklendiyse
                changed_properties.insert(key.clone(), new_val.clone());
            }
        }

        if changed_properties.is_empty() {
            None
        } else {
            Some(InstancePatch {
                syncix_id: self.syncix_id,
                changed_properties,
            })
        }
    }
}
/// Hedef çözümleme sonucu (UUID / kısa UUID / isim ile arama).
pub enum ResolveResult {
    One(Uuid),
    NotFound,
    /// (isim, class_name, uuid) listesi
    Ambiguous(Vec<(String, String, Uuid)>),
}

pub struct DataModel {
    instances: HashMap<Uuid, InstanceNode>,
    root_id: Uuid,
}

impl DataModel {
    pub fn new() -> Self {
        let root = InstanceNode::new("DataModel", "Game");
        let root_id = root.syncix_id;

        let mut instances = HashMap::new();
        instances.insert(root_id, root);

        Self { instances, root_id }
    }

    pub fn get_all_instances(&self) -> &HashMap<Uuid, InstanceNode> {
        &self.instances
    }

    /// Yeni bir instance ekler. Çakışma kontrolü yapar.
    pub fn upsert_instance(&mut self, incoming: InstanceNode) -> Result<(), ModelError> {
        if let Some(existing) = self.instances.get(&incoming.syncix_id) {
            // Conflict Resolution: Gelen veri daha eskiyse reddet
            if incoming.last_updated < existing.last_updated {
                return Err(ModelError::VersionConflict {
                    current: existing.last_updated,
                    incoming: incoming.last_updated,
                });
            }
        }

        // Eğer ebeveyni varsa, ebeveynin children listesine ekle
        if let Some(parent_id) = incoming.parent {
            if let Some(parent) = self.instances.get_mut(&parent_id) {
                if !parent.children.contains(&incoming.syncix_id) {
                    parent.children.push(incoming.syncix_id);
                }
            } else {
                return Err(ModelError::InvalidParent(parent_id));
            }
        }

        self.instances.insert(incoming.syncix_id, incoming);
        Ok(())
    }

    /// Bir düğümün tüm torunlarını (kendisi hariç) toplar.
    fn collect_descendants(&self, id: &Uuid) -> Vec<Uuid> {
        let mut result = Vec::new();
        let mut stack: Vec<Uuid> = self
            .instances
            .get(id)
            .map(|n| n.children.clone())
            .unwrap_or_default();
        // Döngü koruması
        let mut guard = 0;
        while let Some(cur) = stack.pop() {
            result.push(cur);
            if let Some(node) = self.instances.get(&cur) {
                stack.extend(node.children.iter().copied());
            }
            guard += 1;
            if guard > 100_000 {
                break;
            }
        }
        result
    }

    /// Bir objeyi ve TÜM torunlarını siler (cascade). Studio'da Destroy() alt ağacı
    /// da yok ettiği için core modelinin de aynısını yapması gerekir; aksi halde
    /// öksüz (dangling) çocuklar modelde kalıp state ayrışmasına yol açar.
    pub fn remove_instance(&mut self, id: &Uuid) -> Option<InstanceNode> {
        // Önce torunları sil
        for d in self.collect_descendants(id) {
            self.instances.remove(&d);
        }
        // Sonra düğümün kendisini sil ve ebeveyninin children listesinden çıkar
        if let Some(instance) = self.instances.remove(id) {
            if let Some(parent_id) = instance.parent {
                if let Some(parent) = self.instances.get_mut(&parent_id) {
                    parent.children.retain(|&child_id| child_id != *id);
                }
            }
            Some(instance)
        } else {
            None
        }
    }

    /// Bir objeyi yeni bir ebeveyne taşır. Eski ebeveynin children listesinden
    /// çıkarır, yeni ebeveynin listesine ekler ve node'un parent alanını günceller.
    /// Dönen değer: (eski_parent, yeni_parent) — VS Code bildirimi için.
    pub fn reparent(
        &mut self,
        id: &Uuid,
        new_parent: Option<Uuid>,
    ) -> Result<(Option<Uuid>, Option<Uuid>), ModelError> {
        let old_parent = self
            .instances
            .get(id)
            .ok_or(ModelError::InstanceNotFound(*id))?
            .parent;

        if old_parent == new_parent {
            return Ok((old_parent, new_parent));
        }

        // Eski ebeveynin children listesinden çıkar
        if let Some(op) = old_parent {
            if let Some(parent) = self.instances.get_mut(&op) {
                parent.children.retain(|c| c != id);
            }
        }

        // Node'un parent alanını güncelle
        if let Some(inst) = self.instances.get_mut(id) {
            inst.parent = new_parent;
            inst.last_updated = Utc::now().timestamp_millis();
        }

        // Yeni ebeveynin children listesine ekle
        if let Some(np) = new_parent {
            if let Some(parent) = self.instances.get_mut(&np) {
                if !parent.children.contains(id) {
                    parent.children.push(*id);
                }
            }
        }

        Ok((old_parent, new_parent))
    }

    pub fn get_instance(&self, id: &Uuid) -> Option<&InstanceNode> {
        self.instances.get(id)
    }

    pub fn get_mut_instance(&mut self, id: &Uuid) -> Option<&mut InstanceNode> {
        self.instances.get_mut(id)
    }

    /// İsimle instance arar (büyük/küçük harf duyarsız, tam eşleşme).
    /// CLI ve komutlarda UUID yerine isim kullanılabilmesi için.
    pub fn find_by_name(&self, name: &str) -> Vec<Uuid> {
        let lower = name.to_lowercase();
        self.instances
            .iter()
            .filter(|(_, node)| node.class_name != "DataModel" && node.name.to_lowercase() == lower)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Belirli bir ebeveynin, verilen isimdeki çocuklarını döndürür (büyük/küçük harf duyarsız).
    fn children_named(&self, parent: &Uuid, name: &str) -> Vec<Uuid> {
        let lower = name.to_lowercase();
        if let Some(p) = self.instances.get(parent) {
            p.children
                .iter()
                .filter(|cid| {
                    self.instances
                        .get(cid)
                        .map(|c| c.name.to_lowercase() == lower)
                        .unwrap_or(false)
                })
                .copied()
                .collect()
        } else {
            Vec::new()
        }
    }

    /// İsimdeki kök (servis) düğümlerini döndürür.
    fn roots_named(&self, name: &str) -> Vec<Uuid> {
        let lower = name.to_lowercase();
        self.instances
            .iter()
            .filter(|(_, n)| {
                n.parent.is_none()
                    && n.class_name != "DataModel"
                    && n.name.to_lowercase() == lower
            })
            .map(|(id, _)| *id)
            .collect()
    }

    /// Nokta ile ayrılmış yol çözümler: "Workspace.Model.Part" veya "game.Workspace.Baseplate".
    pub fn resolve_path(&self, path: &str) -> ResolveResult {
        let mut segments: Vec<&str> = path.split('.').filter(|s| !s.is_empty()).collect();
        if segments.is_empty() {
            return ResolveResult::NotFound;
        }
        // İsteğe bağlı "game" öneki
        if segments[0].eq_ignore_ascii_case("game") {
            segments.remove(0);
        }
        if segments.is_empty() {
            return ResolveResult::NotFound;
        }

        // İlk segment: kök servis
        let roots = self.roots_named(segments[0]);
        let mut current = match roots.len() {
            1 => roots[0],
            0 => return ResolveResult::NotFound,
            _ => {
                return ResolveResult::Ambiguous(
                    roots
                        .iter()
                        .filter_map(|id| {
                            self.instances
                                .get(id)
                                .map(|n| (n.name.clone(), n.class_name.clone(), *id))
                        })
                        .collect(),
                )
            }
        };

        // Kalan segmentleri çocuklar üzerinden yürü
        for seg in &segments[1..] {
            let matches = self.children_named(&current, seg);
            current = match matches.len() {
                1 => matches[0],
                0 => return ResolveResult::NotFound,
                _ => {
                    return ResolveResult::Ambiguous(
                        matches
                            .iter()
                            .filter_map(|id| {
                                self.instances
                                    .get(id)
                                    .map(|n| (n.name.clone(), n.class_name.clone(), *id))
                            })
                            .collect(),
                    )
                }
            };
        }

        ResolveResult::One(current)
    }

    /// Bir komut hedefini çözümler. Sırasıyla dener:
    /// 1. Nokta içeren yol (Workspace.Model.Part)
    /// 2. Tam UUID
    /// 3. Kısa UUID öneki (en az 6 hane, örn. "d8d0cf78")
    /// 4. İsim (tam eşleşme; tek sonuçsa)
    /// Belirsizlikte adaylar döner ki istemciye anlamlı hata verilebilsin.
    pub fn resolve_target(&self, target: &str) -> ResolveResult {
        // Nokta içeriyorsa yol olarak yorumla (UUID '-' içerir, '.' içermez)
        if target.contains('.') {
            return self.resolve_path(target);
        }
        if let Ok(u) = Uuid::parse_str(target) {
            return if self.instances.contains_key(&u) {
                ResolveResult::One(u)
            } else {
                ResolveResult::NotFound
            };
        }

        let is_hexish = target.len() >= 6
            && target.chars().all(|c| c.is_ascii_hexdigit() || c == '-');
        if is_hexish {
            if let Some(u) = self.find_by_short_uuid(&target.to_lowercase()) {
                return ResolveResult::One(u);
            }
        }

        let matches = self.find_by_name(target);
        match matches.len() {
            0 => ResolveResult::NotFound,
            1 => ResolveResult::One(matches[0]),
            _ => ResolveResult::Ambiguous(
                matches
                    .iter()
                    .filter_map(|id| {
                        self.instances
                            .get(id)
                            .map(|n| (n.name.clone(), n.class_name.clone(), *id))
                    })
                    .collect(),
            ),
        }
    }

    pub fn find_by_short_uuid(&self, short_uuid: &str) -> Option<Uuid> {
        for id in self.instances.keys() {
            if id.to_string().starts_with(short_uuid) {
                return Some(*id);
            }
        }
        None
    }

    /// Sprint 5: Data Integrity & Resync (Consistency Check)
    /// Ağacın bütünlüğünü tarar. Öksüz (dangling) parent veya child referanslarını tespit eder.
    pub fn verify_consistency(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        for (id, node) in &self.instances {
            // Check Parent
            if let Some(parent_id) = node.parent {
                if !self.instances.contains_key(&parent_id) {
                    errors.push(format!(
                        "Node {} ({}) parent {} ID'sini gösteriyor ancak o Parent yok.",
                        node.name, id, parent_id
                    ));
                } else {
                    let parent_node = self.instances.get(&parent_id).unwrap();
                    if !parent_node.children.contains(id) {
                        errors.push(format!("Node {} points at a parent, but parent {} does not list this id among its children.", id, parent_id));
                    }
                }
            }

            // Check Children
            for child_id in &node.children {
                if !self.instances.contains_key(child_id) {
                    errors.push(format!(
                        "Node {} ({}) child {} ID'sine sahip ancak o Child yok.",
                        node.name, id, child_id
                    ));
                } else {
                    let child_node = self.instances.get(child_id).unwrap();
                    if child_node.parent != Some(*id) {
                        errors.push(format!(
                            "Child {} ({}) parent olarak başka bir ID gösteriyor.",
                            child_node.name, child_id
                        ));
                    }
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Thread-safe (Thread-güvenli) DataModel wrapper'ı.
/// Tüm çekirdek servisleri (Watcher, HTTP Server vb.) bu yapıyı paylaşacak.
pub type SharedDataModel = Arc<RwLock<DataModel>>;

pub fn create_shared_model() -> SharedDataModel {
    Arc::new(RwLock::new(DataModel::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test yardımcısı: parent altına isimli bir düğüm ekler ve id'sini döndürür.
    fn add(model: &mut DataModel, class: &str, name: &str, parent: Option<Uuid>) -> Uuid {
        let mut node = InstanceNode::new(class, name);
        node.parent = parent;
        let id = node.syncix_id;
        model.upsert_instance(node).expect("upsert basarisiz");
        id
    }

    /// Bir düğüm silinince TÜM alt ağacı da silinmeli (Studio'daki Destroy davranışı).
    #[test]
    fn test_cascade_delete_removes_descendants() {
        let mut m = DataModel::new();
        let ws = add(&mut m, "Workspace", "Workspace", None);
        let folder = add(&mut m, "Folder", "Klasor", Some(ws));
        let part = add(&mut m, "Part", "Kutu", Some(folder));
        let decal = add(&mut m, "Decal", "Doku", Some(part));

        assert!(m.remove_instance(&folder).is_some());

        assert!(m.get_instance(&folder).is_none(), "klasor silinmeli");
        assert!(m.get_instance(&part).is_none(), "cocuk da silinmeli");
        assert!(m.get_instance(&decal).is_none(), "torun da silinmeli");
        assert!(m.get_instance(&ws).is_some(), "ebeveyn durmali");
        assert!(
            !m.get_instance(&ws).unwrap().children.contains(&folder),
            "ebeveynin children listesi temizlenmeli"
        );
    }

    /// Taşıma: eski ebeveynden çıkmalı, yeni ebeveyne eklenmeli, model tutarlı kalmalı.
    #[test]
    fn test_reparent_updates_both_parents() {
        let mut m = DataModel::new();
        let ws = add(&mut m, "Workspace", "Workspace", None);
        let a = add(&mut m, "Folder", "A", Some(ws));
        let b = add(&mut m, "Folder", "B", Some(ws));
        let part = add(&mut m, "Part", "Kutu", Some(a));

        let (old, new) = m.reparent(&part, Some(b)).expect("reparent basarisiz");
        assert_eq!(old, Some(a));
        assert_eq!(new, Some(b));
        assert_eq!(m.get_instance(&part).unwrap().parent, Some(b));
        assert!(!m.get_instance(&a).unwrap().children.contains(&part));
        assert!(m.get_instance(&b).unwrap().children.contains(&part));
        assert!(m.verify_consistency().is_ok(), "model tutarli kalmali");
    }

    /// KİMLİK KURALI: UUID yalnızca CREATE anında üretilir. Yeniden adlandırma,
    /// taşıma veya tekrar gelen FULL_SYNC onu ASLA değiştirmemeli.
    /// Bu kural bozulursa iki taraf aynı objeyi iki farklı obje sanar ve
    /// senkron sessizce ikizlenir; bu yüzden ayrı ayrı test ediliyor.
    #[test]
    fn uuid_yeniden_adlandirmada_degismez() {
        let mut m = DataModel::new();
        let ws = add(&mut m, "Workspace", "Workspace", None);
        let part = add(&mut m, "Part", "EskiAd", Some(ws));

        m.get_mut_instance(&part).unwrap().name = "YeniAd".to_string();

        assert_eq!(m.get_instance(&part).unwrap().syncix_id, part);
        match m.resolve_target("YeniAd") {
            ResolveResult::One(u) => assert_eq!(u, part, "yeni isim ayni UUID'ye cozulmeli"),
            _ => panic!("yeniden adlandirilan obje bulunamadi"),
        }
        assert!(matches!(m.resolve_target("EskiAd"), ResolveResult::NotFound));
    }

    #[test]
    fn uuid_tasimada_degismez() {
        let mut m = DataModel::new();
        let ws = add(&mut m, "Workspace", "Workspace", None);
        let a = add(&mut m, "Folder", "A", Some(ws));
        let b = add(&mut m, "Folder", "B", Some(ws));
        let part = add(&mut m, "Part", "Kutu", Some(a));

        m.reparent(&part, Some(b)).expect("reparent basarisiz");

        assert_eq!(m.get_instance(&part).unwrap().syncix_id, part);
        assert_eq!(m.get_instance(&part).unwrap().parent, Some(b));
    }

    /// FULL_SYNC her yeniden bağlanmada tüm ağacı yeniden gönderir.
    /// Aynı UUID ile gelen düğüm yeni bir obje yaratmamalı, mevcudu güncellemeli.
    #[test]
    fn full_sync_tekrari_obje_ikizlemez() {
        let mut m = DataModel::new();
        let ws = add(&mut m, "Workspace", "Workspace", None);
        let part = add(&mut m, "Part", "Kutu", Some(ws));
        let onceki_sayi = m.get_instance(&ws).unwrap().children.len();

        // Studio yeniden bağlandı: aynı UUID, güncellenmiş isimle tekrar geliyor.
        let mut tekrar = InstanceNode::new("Part", "KutuYeniAd");
        tekrar.syncix_id = part;
        tekrar.parent = Some(ws);
        m.upsert_instance(tekrar).expect("tekrar upsert basarisiz");

        assert_eq!(
            m.get_instance(&ws).unwrap().children.len(),
            onceki_sayi,
            "ayni UUID ikinci bir cocuk olusturmamali"
        );
        assert_eq!(m.get_instance(&part).unwrap().name, "KutuYeniAd");
        assert!(m.verify_consistency().is_ok());
    }

    /// Hedef çözümleme: nokta-yol, kısa UUID ve isim.
    #[test]
    fn test_resolve_target_path_shortuuid_and_name() {
        let mut m = DataModel::new();
        let ws = add(&mut m, "Workspace", "Workspace", None);
        let folder = add(&mut m, "Folder", "Dekor", Some(ws));
        let part = add(&mut m, "Part", "Sutun", Some(folder));

        // Yol ile
        match m.resolve_target("Workspace.Dekor.Sutun") {
            ResolveResult::One(id) => assert_eq!(id, part),
            _ => panic!("yol cozumlenemedi"),
        }
        // Kısa UUID ile
        let short = &part.to_string()[0..8];
        match m.resolve_target(short) {
            ResolveResult::One(id) => assert_eq!(id, part),
            _ => panic!("kisa uuid cozumlenemedi"),
        }
        // İsim ile (tek eşleşme)
        match m.resolve_target("Sutun") {
            ResolveResult::One(id) => assert_eq!(id, part),
            _ => panic!("isim cozumlenemedi"),
        }
        // Olmayan hedef
        assert!(matches!(m.resolve_target("YokBoyleBirSey"), ResolveResult::NotFound));
    }

    /// Aynı isimde iki kardeş varsa isim çözümlemesi belirsiz olmalı (yanlış objeyi seçmemeli).
    #[test]
    fn test_resolve_target_ambiguous_name() {
        let mut m = DataModel::new();
        let ws = add(&mut m, "Workspace", "Workspace", None);
        let _p1 = add(&mut m, "Part", "Kutu", Some(ws));
        let _p2 = add(&mut m, "Part", "Kutu", Some(ws));

        match m.resolve_target("Kutu") {
            ResolveResult::Ambiguous(list) => assert_eq!(list.len(), 2),
            _ => panic!("belirsizlik tespit edilmeliydi"),
        }
    }

    #[tokio::test]
    async fn test_upsert_and_conflict_resolution() {
        let mut model = DataModel::new();
        let mut node = InstanceNode::new("Part", "TestPart");
        let id = node.syncix_id;

        // İlk ekleme başarılı olmalı
        assert!(model.upsert_instance(node.clone()).is_ok());

        // Eski versiyon ile güncellemeyi dene (Conflict)
        node.last_updated -= 1000;
        node.name = "OldName".to_string();
        let result = model.upsert_instance(node.clone());
        assert!(matches!(result, Err(ModelError::VersionConflict { .. })));

        // İsmin değişmediğini doğrula
        assert_eq!(model.get_instance(&id).unwrap().name, "TestPart");

        // Yeni versiyon ile güncelle
        node.last_updated += 2000;
        node.name = "NewName".to_string();
        assert!(model.upsert_instance(node).is_ok());
        assert_eq!(model.get_instance(&id).unwrap().name, "NewName");
    }

    #[tokio::test]
    async fn test_thread_safety() {
        let shared_model = create_shared_model();
        let node = InstanceNode::new("Part", "ConcurrentPart");
        let id = node.syncix_id;

        // Thread 1: Ekleme yapar
        let model_clone1 = shared_model.clone();
        let node_clone = node.clone();
        let t1 = tokio::spawn(async move {
            let mut lock = model_clone1.write().await;
            lock.upsert_instance(node_clone).unwrap();
        });

        // Thread 2: Okuma yapar (Thread 1 bitmesini bekledikten sonra)
        let model_clone2 = shared_model.clone();
        let t2 = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            let lock = model_clone2.read().await;
            assert!(lock.get_instance(&id).is_some());
        });

        let _ = tokio::join!(t1, t2);
    }
}

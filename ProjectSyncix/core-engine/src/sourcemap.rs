//! sourcemap.json üretimi.
//!
//! Neden gerekli: luau-lsp (VS Code'daki Luau dil sunucusu) hangi dosyanın
//! DataModel'de nereye karşılık geldiğini bilmez. sourcemap.json ona bu haritayı
//! verir; ancak o zaman `game.ReplicatedStorage.Modul` gibi ifadelerde otomatik
//! tamamlama ve tip denetimi çalışır. Rojo'nun en çok kullanılan özelliklerinden
//! biri budur ve Syncix'te eksikti.
//!
//! Biçim Rojo ile aynıdır, dolayısıyla mevcut luau-lsp kurulumları hiçbir
//! değişiklik gerektirmeden çalışır:
//! { "name": ..., "className": ..., "filePaths": [...], "children": [...] }

use crate::layout;
use crate::model::DataModel;
use serde::Serialize;
use std::path::Path;
use uuid::Uuid;

#[derive(Serialize)]
pub struct SourcemapNode {
    pub name: String,
    #[serde(rename = "className")]
    pub class_name: String,
    #[serde(rename = "filePaths", skip_serializing_if = "Vec::is_empty")]
    pub file_paths: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<SourcemapNode>,
}

/// Yollar sourcemap.json'un bulunduğu dizine göre ve daima ileri eğik çizgiyle
/// yazılır; luau-lsp Windows'ta da bu biçimi bekler.
fn goreceli(yol: &Path, kok: &Path) -> String {
    let p = yol.strip_prefix(kok).unwrap_or(yol);
    p.to_string_lossy().replace('\\', "/")
}

fn dugum(dm: &DataModel, uuid: &Uuid, sync_dir: &str, kok: &Path) -> Option<SourcemapNode> {
    let node = dm.get_instance(uuid)?;

    let mut file_paths = Vec::new();
    if let Some(p) = layout::data_file(dm, sync_dir, uuid) {
        file_paths.push(goreceli(&p, kok));
    }

    let mut children: Vec<SourcemapNode> = node
        .children
        .iter()
        .filter_map(|cid| dugum(dm, cid, sync_dir, kok))
        .collect();
    children.sort_by(|a, b| a.name.cmp(&b.name));

    Some(SourcemapNode {
        name: node.name.clone(),
        class_name: node.class_name.clone(),
        file_paths,
        children,
    })
}

/// Tüm ağacı Rojo uyumlu sourcemap ağacına çevirir.
/// Kök daima DataModel'dir; servisler onun çocuklarıdır.
pub fn olustur(dm: &DataModel, sync_dir: &str, kok: &Path) -> SourcemapNode {
    let mut children: Vec<SourcemapNode> = dm
        .get_all_instances()
        .iter()
        .filter(|(_, n)| n.parent.is_none() && n.class_name != "DataModel")
        .filter_map(|(uuid, _)| dugum(dm, uuid, sync_dir, kok))
        .collect();
    children.sort_by(|a, b| a.name.cmp(&b.name));

    SourcemapNode {
        name: "Root".to_string(),
        class_name: "DataModel".to_string(),
        file_paths: Vec::new(),
        children,
    }
}

pub fn json(dm: &DataModel, sync_dir: &str, kok: &Path) -> String {
    serde_json::to_string_pretty(&olustur(dm, sync_dir, kok)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::InstanceNode;

    fn ekle(m: &mut DataModel, class: &str, name: &str, parent: Option<Uuid>) -> Uuid {
        let mut n = InstanceNode::new(class, name);
        n.parent = parent;
        let id = n.syncix_id;
        m.upsert_instance(n).unwrap();
        id
    }

    #[test]
    fn agac_yapisi_ve_sinif_adlari_korunur() {
        let mut m = DataModel::new();
        let rs = ekle(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
        ekle(&mut m, "ModuleScript", "Modul", Some(rs));

        let sm = olustur(&m, "src_workspace", Path::new("."));
        assert_eq!(sm.class_name, "DataModel");
        assert_eq!(sm.children.len(), 1);

        let servis = &sm.children[0];
        assert_eq!(servis.name, "ReplicatedStorage");
        assert_eq!(servis.children.len(), 1);
        assert_eq!(servis.children[0].name, "Modul");
        assert_eq!(servis.children[0].class_name, "ModuleScript");
    }

    #[test]
    fn yollar_ileri_egik_cizgi_kullanir() {
        let mut m = DataModel::new();
        let rs = ekle(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
        ekle(&mut m, "ModuleScript", "Modul", Some(rs));

        let sm = olustur(&m, "src_workspace", Path::new("."));
        let yol = &sm.children[0].children[0].file_paths[0];
        assert!(!yol.contains('\\'), "ters egik cizgi olmamali: {}", yol);
        assert!(yol.ends_with("Modul.lua"), "beklenmeyen yol: {}", yol);
    }

    #[test]
    fn cocuklar_isme_gore_sirali() {
        let mut m = DataModel::new();
        let ws = ekle(&mut m, "Workspace", "Workspace", None);
        ekle(&mut m, "Part", "Zebra", Some(ws));
        ekle(&mut m, "Part", "Alfa", Some(ws));

        let sm = olustur(&m, "src_workspace", Path::new("."));
        let cocuklar = &sm.children[0].children;
        assert_eq!(cocuklar[0].name, "Alfa");
        assert_eq!(cocuklar[1].name, "Zebra");
    }
}

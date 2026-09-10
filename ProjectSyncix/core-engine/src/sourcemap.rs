//! sourcemap.json üretimi.
//!
//! Neden gerekli: luau-lsp (VS Code'daki Luau dil sunucusu) hangi dosyanın
//! DataModel'de nereye karşılık geldiğini bilmez. sourcemap.json ona bu haritayı
//! verir; ancak o zaman `game.ReplicatedStorage.Modul` gibi ifadelerde otomatik
//! tamamlama ve type_name denetimi çalışır. Rojo'nun en çok kullanılan özelliklerinden
//! biri budur ve Syncix'te eksikti.
//!
//! Biçim Rojo ile aynıdır, dolayısıyla current_value luau-lsp kurulumları hiçbir
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
fn relative_path(fs_path: &Path, root_dir: &Path) -> String {
    let p = fs_path.strip_prefix(root_dir).unwrap_or(fs_path);
    p.to_string_lossy().replace('\\', "/")
}

fn node_entry(dm: &DataModel, uuid: &Uuid, sync_dir: &str, root_dir: &Path) -> Option<SourcemapNode> {
    let node = dm.get_instance(uuid)?;

    let mut file_paths = Vec::new();
    if let Some(p) = layout::data_file(dm, sync_dir, uuid) {
        file_paths.push(relative_path(&p, root_dir));
    }

    let mut children: Vec<SourcemapNode> = node
        .children
        .iter()
        .filter_map(|cid| node_entry(dm, cid, sync_dir, root_dir))
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
/// Kök daima DataModel'dir; service_list onun çocuklarıdır.
pub fn generate(dm: &DataModel, sync_dir: &str, root_dir: &Path) -> SourcemapNode {
    let mut children: Vec<SourcemapNode> = dm
        .get_all_instances()
        .iter()
        .filter(|(_, n)| n.parent.is_none() && n.class_name != "DataModel")
        .filter_map(|(uuid, _)| node_entry(dm, uuid, sync_dir, root_dir))
        .collect();
    children.sort_by(|a, b| a.name.cmp(&b.name));

    SourcemapNode {
        name: "Root".to_string(),
        class_name: "DataModel".to_string(),
        file_paths: Vec::new(),
        children,
    }
}

pub fn json(dm: &DataModel, sync_dir: &str, root_dir: &Path) -> String {
    serde_json::to_string_pretty(&generate(dm, sync_dir, root_dir)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::InstanceNode;

    fn add_instance(m: &mut DataModel, class: &str, name: &str, parent: Option<Uuid>) -> Uuid {
        let mut n = InstanceNode::new(class, name);
        n.parent = parent;
        let id = n.syncix_id;
        m.upsert_instance(n).unwrap();
        id
    }

    #[test]
    fn tree_shape_and_class_names_are_kept() {
        let mut m = DataModel::new();
        let rs = add_instance(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
        add_instance(&mut m, "ModuleScript", "Modul", Some(rs));

        let sm = generate(&m, "src_workspace", Path::new("."));
        assert_eq!(sm.class_name, "DataModel");
        assert_eq!(sm.children.len(), 1);

        let service_name = &sm.children[0];
        assert_eq!(service_name.name, "ReplicatedStorage");
        assert_eq!(service_name.children.len(), 1);
        assert_eq!(service_name.children[0].name, "Modul");
        assert_eq!(service_name.children[0].class_name, "ModuleScript");
    }

    #[test]
    fn paths_use_forward_slashes() {
        let mut m = DataModel::new();
        let rs = add_instance(&mut m, "ReplicatedStorage", "ReplicatedStorage", None);
        add_instance(&mut m, "ModuleScript", "Modul", Some(rs));

        let sm = generate(&m, "src_workspace", Path::new("."));
        let fs_path = &sm.children[0].children[0].file_paths[0];
        assert!(!fs_path.contains('\\'), "ters egik cizgi olmamali: {}", fs_path);
        assert!(fs_path.ends_with("Modul.lua"), "beklenmeyen yol: {}", fs_path);
    }

    #[test]
    fn children_sorted_by_name() {
        let mut m = DataModel::new();
        let ws = add_instance(&mut m, "Workspace", "Workspace", None);
        add_instance(&mut m, "Part", "Zebra", Some(ws));
        add_instance(&mut m, "Part", "Alfa", Some(ws));

        let sm = generate(&m, "src_workspace", Path::new("."));
        let child_entries = &sm.children[0].children;
        assert_eq!(child_entries[0].name, "Alfa");
        assert_eq!(child_entries[1].name, "Zebra");
    }
}

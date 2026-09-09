use crate::model::InstanceNode;
use std::collections::VecDeque;
use std::sync::RwLock;

/// Snapshot Yöneticisi
/// Verilen DataModel'in (Kök InstanceNode) derin kopyalarını belirli aralıklarla alır.
/// Geçmişe dönük kopyaları limitli bir kuyrukta tutar (Undo/Redo ve Crash Recovery için).
pub struct SnapshotManager {
    /// Undo History (Eski versiyonlar)
    history: RwLock<VecDeque<InstanceNode>>,

    /// Redo History (İleri alınan versiyonlar)
    future: RwLock<VecDeque<InstanceNode>>,

    /// Maksimum tutulacak snapshot sayısı
    max_snapshots: usize,
}

impl SnapshotManager {
    pub fn new(max_snapshots: usize) -> Self {
        Self {
            history: RwLock::new(VecDeque::with_capacity(max_snapshots)),
            future: RwLock::new(VecDeque::new()),
            max_snapshots,
        }
    }

    /// O anki DataModel ağacını Snapshot olarak kaydeder.
    pub fn take_snapshot(&self, current_root: &InstanceNode) {
        let mut history = self.history.write().unwrap();

        // Kapasite dolduysa en eskisini at
        if history.len() >= self.max_snapshots {
            history.pop_front();
        }

        // Ağacın Derin Kopyasını (Deep Clone) alıp kaydet
        history.push_back(current_root.clone());

        // Yeni değişiklik yapıldığında Redo kuyruğu temizlenir
        self.future.write().unwrap().clear();
    }

    /// Bir önceki snapshot'a döner (Undo)
    pub fn undo(&self, current_root: &InstanceNode) -> Option<InstanceNode> {
        let mut history = self.history.write().unwrap();
        if let Some(previous_state) = history.pop_back() {
            // Şimdiki durumu future kuyruğuna at
            let mut future = self.future.write().unwrap();
            future.push_front(current_root.clone());

            return Some(previous_state);
        }
        None
    }

    /// İleri sarılan bir snapshot'ı geri getirir (Redo)
    pub fn redo(&self, current_root: &InstanceNode) -> Option<InstanceNode> {
        let mut future = self.future.write().unwrap();
        if let Some(next_state) = future.pop_front() {
            // Şimdiki durumu history kuyruğuna at
            let mut history = self.history.write().unwrap();
            history.push_back(current_root.clone());

            return Some(next_state);
        }
        None
    }

    /// Kaç adet kayıtlı snapshot olduğunu döndürür.
    pub fn history_count(&self) -> usize {
        self.history.read().unwrap().len()
    }
}

use super::transaction::Transaction;
use crate::model::SharedDataModel;

pub struct CommandHistoryManager {
    undo_stack: Vec<Transaction>,
    redo_stack: Vec<Transaction>,
    max_history: usize,
}

impl CommandHistoryManager {
    pub fn new(max_history: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_history,
        }
    }

    /// Yeni bir transaction tamamlandığında history'e ekler
    pub fn push_transaction(&mut self, tx: Transaction) {
        if !tx.is_committed {
            return; // Sadece başarılı işlemler stack'e girer
        }

        // Yeni bir işlem yapıldığında redo stack temizlenir (Standart IDE davranışı)
        self.redo_stack.clear();

        self.undo_stack.push(tx);

        if self.undo_stack.len() > self.max_history {
            self.undo_stack.remove(0);
        }
    }

    pub async fn undo(&mut self, model: &SharedDataModel) -> Result<(), String> {
        if let Some(mut tx) = self.undo_stack.pop() {
            tx.rollback(model).await?;
            self.redo_stack.push(tx);
            Ok(())
        } else {
            Err("Geri alınacak işlem yok (Undo stack boş).".to_string())
        }
    }

    pub async fn redo(&mut self, model: &SharedDataModel) -> Result<(), String> {
        if let Some(mut tx) = self.redo_stack.pop() {
            tx.redo(model).await?;
            self.undo_stack.push(tx);
            Ok(())
        } else {
            Err("Yinelenecak işlem yok (Redo stack boş).".to_string())
        }
    }
}

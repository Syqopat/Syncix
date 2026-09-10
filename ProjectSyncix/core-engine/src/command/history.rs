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

    /// Adds a completed transaction to the history
    pub fn push_transaction(&mut self, tx: Transaction) {
        if !tx.is_committed {
            return; // only successful operations go onto the stack
        }

        // A new operation clears the redo stack (standard IDE behaviour)
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
            Err("Nothing to undo (the undo stack is empty).".to_string())
        }
    }

    pub async fn redo(&mut self, model: &SharedDataModel) -> Result<(), String> {
        if let Some(mut tx) = self.redo_stack.pop() {
            tx.redo(model).await?;
            self.undo_stack.push(tx);
            Ok(())
        } else {
            Err("Nothing to redo (the redo stack is empty).".to_string())
        }
    }
}

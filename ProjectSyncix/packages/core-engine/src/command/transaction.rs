use super::audit::AuditLogger;
use super::types::Command;
use crate::model::SharedDataModel;
use std::time::Instant;
use uuid::Uuid;

/// A Transaction groups multiple commands together to execute them atomically.
pub struct Transaction {
    pub transaction_id: Uuid,
    pub author_id: String, // Treat as Session ID
    pub commands: Vec<Box<dyn Command>>,
    pub is_committed: bool,
}

impl Transaction {
    pub fn new(author_id: &str) -> Self {
        Self {
            transaction_id: Uuid::new_v4(),
            author_id: author_id.to_string(),
            commands: Vec::new(),
            is_committed: false,
        }
    }

    pub fn add_command(&mut self, cmd: Box<dyn Command>) {
        self.commands.push(cmd);
    }

    pub async fn commit(&mut self, model: &SharedDataModel) -> Result<(), String> {
        if self.is_committed {
            return Err("Transaction already committed".to_string());
        }

        let start_time = Instant::now();

        // Validate all commands first
        for cmd in &self.commands {
            if let Err(e) = cmd.validate(model) {
                AuditLogger::log_transaction(
                    self.transaction_id,
                    &self.author_id,
                    self.commands.len(),
                    start_time.elapsed(),
                    "ValidationFailed",
                );
                return Err(e);
            }
        }

        // Execute all
        let mut executed_count = 0;
        for cmd in &mut self.commands {
            match cmd.execute(model).await {
                Ok(_) => executed_count += 1,
                Err(e) => {
                    // Rollback previously executed commands in this transaction
                    for r_cmd in self.commands.iter_mut().take(executed_count).rev() {
                        let _ = r_cmd.undo(model).await;
                    }
                    let reason = format!(
                        "Transaction Failed at index {}, rolled back. Reason: {}",
                        executed_count, e
                    );
                    AuditLogger::log_transaction(
                        self.transaction_id,
                        &self.author_id,
                        self.commands.len(),
                        start_time.elapsed(),
                        "RolledBack",
                    );
                    return Err(reason);
                }
            }
        }

        self.is_committed = true;
        AuditLogger::log_transaction(
            self.transaction_id,
            &self.author_id,
            self.commands.len(),
            start_time.elapsed(),
            "Applied",
        );
        Ok(())
    }

    pub async fn rollback(&mut self, model: &SharedDataModel) -> Result<(), String> {
        if !self.is_committed {
            return Err("Cannot rollback an uncommitted transaction".to_string());
        }

        // Undo in reverse order
        for cmd in self.commands.iter_mut().rev() {
            cmd.undo(model).await?;
        }

        self.is_committed = false;
        Ok(())
    }

    pub async fn redo(&mut self, model: &SharedDataModel) -> Result<(), String> {
        if self.is_committed {
            return Err("Cannot redo an already committed transaction".to_string());
        }

        for cmd in &mut self.commands {
            cmd.redo(model).await?;
        }

        self.is_committed = true;
        Ok(())
    }
}

use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum OperationType {
    CreateInstance,
    DeleteInstance,
    ModifyProperty,
    ModifyScript,
    UploadAsset,
}

pub struct OperationPermissionManager {
    // author_id -> set of allowed operations
    pub permissions: HashMap<String, HashSet<OperationType>>,
}

impl OperationPermissionManager {
    pub fn new() -> Self {
        Self {
            permissions: HashMap::new(),
        }
    }

    /// Grants a permission to a new author (plugin, user, agent, ...).
    pub fn grant(&mut self, author_id: &str, operation: OperationType) {
        let perms = self.permissions.entry(author_id.to_string()).or_default();
        perms.insert(operation);
    }

    /// Revokes a specific permission from an author.
    pub fn revoke(&mut self, author_id: &str, operation: &OperationType) {
        if let Some(perms) = self.permissions.get_mut(author_id) {
            perms.remove(operation);
        }
    }

    /// Checks whether an author may perform a given operation.
    pub fn can_execute(&self, author_id: &str, operation: &OperationType) -> Result<(), String> {
        // An author "SYSTEM" or "ROOT" could always be allowed (optional)
        if author_id == "SYSTEM" {
            return Ok(());
        }

        if let Some(perms) = self.permissions.get(author_id) {
            if perms.contains(operation) {
                return Ok(());
            }
        }

        Err(format!("Permission denied: '{}' is not allowed to perform '{:?}'.", author_id, operation))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operation_permissions() {
        let mut manager = OperationPermissionManager::new();

        let author = "Plugin_A";
        manager.grant(author, OperationType::ModifyProperty);
        manager.grant(author, OperationType::CreateInstance);

        assert!(manager
            .can_execute(author, &OperationType::ModifyProperty)
            .is_ok());
        assert!(manager
            .can_execute(author, &OperationType::CreateInstance)
            .is_ok());

        // An operation that was never granted must be rejected
        let result = manager.can_execute(author, &OperationType::DeleteInstance);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Permission denied"));

        // SYSTEM author test
        assert!(manager
            .can_execute("SYSTEM", &OperationType::DeleteInstance)
            .is_ok());
    }
}

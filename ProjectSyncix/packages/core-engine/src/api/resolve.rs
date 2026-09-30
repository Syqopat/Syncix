use uuid::Uuid;

use crate::api::errors::{ApiError, ApiResult};
use crate::model::{DataModel, ResolveResult};

/// "<label> not found: <target>", with the closest targets that do exist.
pub fn not_found_message(dm: &DataModel, label: &str, target: &str) -> String {
    let mut message = format!("{} not found: {}", label, target);
    if let Some(hint) = crate::suggest::did_you_mean(&dm.target_suggestions(target)) {
        message.push('\n');
        message.push_str(&hint);
    }
    message
}

fn ambiguous_message(label: &str, target: &str, candidates: &[(String, String, Uuid)]) -> String {
    let list = candidates
        .iter()
        .map(|(name, class_name, id)| format!("  {} ({}) -> {}", name, class_name, &id.to_string()[0..8]))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "{} '{}' is ambiguous: {} matches. Target it with the short UUID:\n{}",
        label,
        target,
        candidates.len(),
        list
    )
}

/// Resolves one target (name, UUID or short UUID), or says why it could not be.
///
/// Every handler that takes a target used to carry its own copy of this match, each
/// with slightly different wording and status codes.
pub fn resolve_one(dm: &DataModel, label: &str, target: &str) -> ApiResult<Uuid> {
    if target.is_empty() {
        return Err(ApiError::BadRequest(format!("{} is required.", label)));
    }
    match dm.resolve_target(target) {
        ResolveResult::One(uuid) => Ok(uuid),
        ResolveResult::NotFound => Err(ApiError::NotFound(not_found_message(dm, label, target))),
        ResolveResult::Ambiguous(candidates) => {
            Err(ApiError::Conflict(ambiguous_message(label, target, &candidates)))
        }
    }
}

/// Resolves a target that a request may leave out: `None` means "not given".
pub fn resolve_optional(dm: &DataModel, label: &str, target: &str) -> ApiResult<Option<Uuid>> {
    if target.is_empty() {
        return Ok(None);
    }
    resolve_one(dm, label, target).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_target_is_a_bad_request() {
        let dm = DataModel::new();
        let error = resolve_one(&dm, "Target", "").unwrap_err();
        assert_eq!(error.status(), axum::http::StatusCode::BAD_REQUEST);
    }

    #[test]
    fn an_unknown_target_is_not_found() {
        let dm = DataModel::new();
        let error = resolve_one(&dm, "Target", "NoSuchThing").unwrap_err();
        assert_eq!(error.status(), axum::http::StatusCode::NOT_FOUND);
        assert!(error.message().contains("NoSuchThing"));
    }

    #[test]
    fn a_missing_optional_target_is_none() {
        let dm = DataModel::new();
        assert!(resolve_optional(&dm, "Parent", "").unwrap().is_none());
    }
}

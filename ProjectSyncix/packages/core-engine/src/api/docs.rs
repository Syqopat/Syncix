use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};
use utoipa::{Modify, OpenApi};

use crate::api::{auth, routes};

/// The API description clients read, generated from the handlers themselves so it
/// cannot drift away from the code.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Syncix Core",
        description = "Local HTTP API of the Syncix core. Bound to 127.0.0.1; every call except /health needs the project token.",
        license(name = "MIT")
    ),
    paths(
        routes::health::status,
        routes::sync::poll,
        routes::sync::push,
        routes::commands::run,
        routes::model::tree,
        routes::model::object,
        routes::model::verify,
        routes::model::sourcemap,
        routes::model::export,
        routes::core::stop,
    ),
    tags(
        (name = "core", description = "Identity and lifetime"),
        (name = "sync", description = "The channel the Studio plugin uses"),
        (name = "commands", description = "Changes asked for by the CLI and the editor"),
        (name = "model", description = "Reading the tree")
    ),
    modifiers(&TokenScheme)
)]
pub struct ApiDoc;

struct TokenScheme;

impl Modify for TokenScheme {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "token",
                SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new(auth::HEADER))),
            );
        }
    }
}

//! Split out of rbxmx_import.rs.

#[allow(unused_imports)]
use super::*;

/// Services a place file carries at its top level. There is exactly one of each in a
/// place and `Instance.new` cannot create them, so an import must merge into the
/// existing one instead of creating a copy.
pub(crate) const SERVICE_CLASSES: &[&str] = &[
    "Workspace",
    "Players",
    "Lighting",
    "MaterialService",
    "ReplicatedFirst",
    "ReplicatedStorage",
    "ServerScriptService",
    "ServerStorage",
    "StarterGui",
    "StarterPack",
    "StarterPlayer",
    "Teams",
    "SoundService",
    "Chat",
    "TextChatService",
    "LocalizationService",
    "TestService",
    "VoiceChatService",
    "ProximityPromptService",
    "HttpService",
    "InsertService",
    "CollectionService",
];

/// Containers that exist once under their parent and cannot be created either.
/// TextChatService's four configurations were missing here: an import tried to create
/// copies, Studio refused, and a UIGradient inside one waited for a parent forever.
pub(crate) const SINGLETON_CHILD_CLASSES: &[&str] = &[
    "StarterPlayerScripts",
    "StarterCharacterScripts",
    "Terrain",
    "BubbleChatConfiguration",
    "ChatWindowConfiguration",
    "ChatInputBarConfiguration",
    "ChannelTabsConfiguration",
];

pub fn is_service(class_name: &str) -> bool {
    SERVICE_CLASSES.contains(&class_name)
}

pub fn service_names() -> impl Iterator<Item = &'static str> {
    SERVICE_CLASSES.iter().copied()
}

/// True for every class an import must map onto an existing instance rather than create.
pub fn is_singleton(class_name: &str) -> bool {
    is_service(class_name) || SINGLETON_CHILD_CLASSES.contains(&class_name)
}

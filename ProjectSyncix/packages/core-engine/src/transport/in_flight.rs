//! Split out of transport.rs.

#[allow(unused_imports)]
use super::*;

/// What payloads ask Studio to create or change: the part of the core's model Studio
/// may not have yet.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct InFlight {
    pub creates: HashSet<Uuid>,
    /// Property changes; a rename is recorded as "Name", a script edit as "Source".
    pub properties: HashSet<(Uuid, String)>,
    pub reparents: HashSet<Uuid>,
    pub destroys: HashSet<Uuid>,
    pub attributes: HashSet<(Uuid, String)>,
    pub tags: HashSet<Uuid>,
}

impl InFlight {
    pub(crate) fn is_empty(&self) -> bool {
        self.creates.is_empty()
            && self.properties.is_empty()
            && self.reparents.is_empty()
            && self.destroys.is_empty()
            && self.attributes.is_empty()
            && self.tags.is_empty()
    }

    pub(crate) fn extend(&mut self, other: &InFlight) {
        self.creates.extend(other.creates.iter().copied());
        self.properties.extend(other.properties.iter().cloned());
        self.reparents.extend(other.reparents.iter().copied());
        self.destroys.extend(other.destroys.iter().copied());
        self.attributes.extend(other.attributes.iter().cloned());
        self.tags.extend(other.tags.iter().copied());
    }
}

/// Records what one payload creates or changes in Studio. Every kind of change the core
/// applies to its model before Studio does is recorded: tracking only creates and
/// property changes let a FULL_SYNC quietly undo renames, moves, deletions, attributes
/// and tags that were still on their way.
pub(crate) fn record_touched(payload: &Payload, into: &mut InFlight) {
    let id_of = |v: &serde_json::Value| {
        v.get("syncix_id")
            .or_else(|| v.get("id"))
            .and_then(|x| x.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
    };
    match payload.event_type {
        EventType::CompositeUpdate => {
            let Some(patches) = payload.data.get("patches").and_then(|x| x.as_array()) else {
                return;
            };
            for patch in patches {
                let Some(data) = patch.get("data") else { continue };
                let Some(id) = id_of(data) else { continue };
                match patch.get("event_type").and_then(|x| x.as_str()) {
                    Some("CREATE") => {
                        into.creates.insert(id);
                    }
                    Some("PROPERTY_UPDATE") => {
                        if let Some(property) = data.get("property").and_then(|x| x.as_str()) {
                            into.properties.insert((id, property.to_string()));
                        }
                    }
                    Some("RENAME_INSTANCE") | Some("RENAME") => {
                        into.properties.insert((id, "Name".to_string()));
                    }
                    Some("REPARENT") => {
                        into.reparents.insert(id);
                    }
                    Some("DESTROY") => {
                        into.destroys.insert(id);
                    }
                    Some("ATTRIBUTE_UPDATE") => {
                        if let Some(name) = data.get("name").and_then(|x| x.as_str()) {
                            into.attributes.insert((id, name.to_string()));
                        }
                    }
                    Some("TAGS_UPDATE") => {
                        into.tags.insert(id);
                    }
                    _ => {}
                }
            }
        }
        // A whole node sent from disk: Studio creates it if it does not have it yet.
        EventType::PushUpdate => {
            if let Some(id) = id_of(&payload.data) {
                into.creates.insert(id);
            }
        }
        _ => {}
    }
}

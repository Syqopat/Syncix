//! DataModel methods split out of model.rs.

#[allow(unused_imports)]
use super::*;

impl DataModel {
    /// Finds instances by name (case-insensitive, exact match).
    /// So the CLI and commands can use a name instead of a UUID.
    pub fn find_by_name(&self, name: &str) -> Vec<Uuid> {
        let lower = name.to_lowercase();
        self.instances
            .iter()
            .filter(|(_, node)| node.class_name != "DataModel" && node.name.to_lowercase() == lower)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Returns the children of a given parent with the given name (case-insensitive).
    fn children_named(&self, parent: &Uuid, name: &str) -> Vec<Uuid> {
        let lower = name.to_lowercase();
        if let Some(p) = self.instances.get(parent) {
            p.children
                .iter()
                .filter(|cid| {
                    self.instances
                        .get(cid)
                        .map(|c| c.name.to_lowercase() == lower)
                        .unwrap_or(false)
                })
                .copied()
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Returns the root (service) nodes with the given name.
    fn roots_named(&self, name: &str) -> Vec<Uuid> {
        let lower = name.to_lowercase();
        self.instances
            .iter()
            .filter(|(_, n)| {
                n.parent.is_none()
                    && n.class_name != "DataModel"
                    && n.name.to_lowercase() == lower
            })
            .map(|(id, _)| *id)
            .collect()
    }

    /// Resolves a dotted path: "Workspace.Model.Part" or "game.Workspace.Baseplate".
    pub fn resolve_path(&self, path: &str) -> ResolveResult {
        let mut segments: Vec<&str> = path.split('.').filter(|s| !s.is_empty()).collect();
        if segments.is_empty() {
            return ResolveResult::NotFound;
        }
        // Optional "game" prefix
        if segments[0].eq_ignore_ascii_case("game") {
            segments.remove(0);
        }
        if segments.is_empty() {
            return ResolveResult::NotFound;
        }

        // First segment: the root service
        let roots = self.roots_named(segments[0]);
        let mut current = match roots.len() {
            1 => roots[0],
            0 => return ResolveResult::NotFound,
            _ => {
                return ResolveResult::Ambiguous(
                    roots
                        .iter()
                        .filter_map(|id| {
                            self.instances
                                .get(id)
                                .map(|n| (n.name.clone(), n.class_name.clone(), *id))
                        })
                        .collect(),
                )
            }
        };

        // Walk the remaining segments through the children
        for seg in &segments[1..] {
            let matches = self.children_named(&current, seg);
            current = match matches.len() {
                1 => matches[0],
                0 => return ResolveResult::NotFound,
                _ => {
                    return ResolveResult::Ambiguous(
                        matches
                            .iter()
                            .filter_map(|id| {
                                self.instances
                                    .get(id)
                                    .map(|n| (n.name.clone(), n.class_name.clone(), *id))
                            })
                            .collect(),
                    )
                }
            };
        }

        ResolveResult::One(current)
    }

    /// Resolves a command target. Tries, in order:
    /// 1. A dotted path (Workspace.Model.Part)
    /// 2. A full UUID
    /// 3. A short UUID prefix (at least 6 characters, e.g. "d8d0cf78")
    /// 4. A name (exact match; if there is a single result)
    ///
    /// On ambiguity the candidates are returned so the client can give a meaningful error.
    pub fn resolve_target(&self, target: &str) -> ResolveResult {
        // Treat it as a path if it contains a dot (a UUID contains '-', never '.')
        if target.contains('.') {
            return self.resolve_path(target);
        }
        if let Ok(u) = Uuid::parse_str(target) {
            return if self.instances.contains_key(&u) {
                ResolveResult::One(u)
            } else {
                ResolveResult::NotFound
            };
        }

        let is_hexish = target.len() >= 6
            && target.chars().all(|c| c.is_ascii_hexdigit() || c == '-');
        if is_hexish {
            if let Some(u) = self.find_by_short_uuid(&target.to_lowercase()) {
                return ResolveResult::One(u);
            }
        }

        let matches = self.find_by_name(target);
        match matches.len() {
            0 => ResolveResult::NotFound,
            1 => ResolveResult::One(matches[0]),
            _ => ResolveResult::Ambiguous(
                matches
                    .iter()
                    .filter_map(|id| {
                        self.instances
                            .get(id)
                            .map(|n| (n.name.clone(), n.class_name.clone(), *id))
                    })
                    .collect(),
            ),
        }
    }

    /// Targets close to one that resolved to nothing, spelt the way the CLI takes them.
    /// For a dotted path the failing segment is compared with the children really there
    /// ("Workspace.Tycons" -> "Workspace.Tycoons"); otherwise with every instance name.
    pub fn target_suggestions(&self, target: &str) -> Vec<String> {
        if !target.contains('.') {
            let mut names: Vec<&str> = self
                .instances
                .values()
                .filter(|n| n.class_name != "DataModel")
                .map(|n| n.name.as_str())
                .collect();
            names.sort_unstable();
            names.dedup();
            return crate::suggest::closest(target, names).into_iter().map(str::to_string).collect();
        }

        let mut segments: Vec<&str> = target.split('.').filter(|s| !s.is_empty()).collect();
        if segments.first().is_some_and(|s| s.eq_ignore_ascii_case("game")) {
            segments.remove(0);
        }
        let mut found_path: Vec<&str> = Vec::new();
        let mut parent: Option<Uuid> = None;
        for segment in segments {
            let children: Vec<(Uuid, &InstanceNode)> = match parent {
                None => self
                    .instances
                    .iter()
                    .filter(|(_, n)| n.parent.is_none() && n.class_name != "DataModel")
                    .map(|(id, n)| (*id, n))
                    .collect(),
                Some(p) => self
                    .instances
                    .get(&p)
                    .map(|n| n.children.iter().filter_map(|c| self.instances.get(c).map(|x| (*c, x))).collect())
                    .unwrap_or_default(),
            };
            let exact: Vec<&(Uuid, &InstanceNode)> =
                children.iter().filter(|(_, n)| n.name.eq_ignore_ascii_case(segment)).collect();
            match exact.as_slice() {
                [(id, node)] => {
                    found_path.push(node.name.as_str());
                    parent = Some(*id);
                }
                // Several with that name: ambiguous, not misspelt.
                [_, _, ..] => return Vec::new(),
                [] => {
                    let names: Vec<&str> = children.iter().map(|(_, n)| n.name.as_str()).collect();
                    return crate::suggest::closest(segment, names)
                        .into_iter()
                        .map(|name| found_path.iter().copied().chain([name]).collect::<Vec<_>>().join("."))
                        .collect();
                }
            }
        }
        Vec::new()
    }

    pub fn find_by_short_uuid(&self, short_uuid: &str) -> Option<Uuid> {
        for id in self.instances.keys() {
            if id.to_string().starts_with(short_uuid) {
                return Some(*id);
            }
        }
        None
    }
}

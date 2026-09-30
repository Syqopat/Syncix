//! Split out of project.rs.

#[allow(unused_imports)]
use super::*;

/// Which directions sync is active in.
///
/// Rojo works one-way: the file system is the single source of truth and Studio only
/// receives. Syncix is two-way by default, but not everyone wants that —
/// someone on a team may want Studio read-only, while a designer may not want
/// the disk overwritten. So the direction is a setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncMode {
    /// Both directions enabled. The default.
    TwoWay,
    /// Studio -> disk. Changes made in Studio are written to disk; changes on disk
    /// do NOT go to Studio. For people who design scenes in Studio and want the
    /// result in version control.
    StudioToDisk,
    /// Disk -> Studio. Rojo-style workflow: the file system is the source of truth.
    DiskToStudio,
    /// No direction is automatic; only explicitly given commands are carried out
    /// (syncix pull, syncix set, ...). For working under supervision on a risky scene.
    Manual,
}

impl SyncMode {
    pub(crate) fn resolve_arg(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "two_way" | "twoway" | "both" => Some(Self::TwoWay),
            "studio_to_disk" | "studio" | "pull" => Some(Self::StudioToDisk),
            "disk_to_studio" | "disk" | "push" | "rojo" => Some(Self::DiskToStudio),
            "manual" | "off" | "none" => Some(Self::Manual),
            _ => None,
        }
    }

    pub fn name_of(&self) -> &'static str {
        match self {
            Self::TwoWay => "two_way",
            Self::StudioToDisk => "studio_to_disk",
            Self::DiskToStudio => "disk_to_studio",
            Self::Manual => "manual",
        }
    }

    /// Should a change made in Studio be reflected in the model and on disk?
    pub fn accepts_from_studio(&self) -> bool {
        matches!(self, Self::TwoWay | Self::StudioToDisk)
    }

    /// Should a change made on disk be sent to Studio?
    pub fn accepts_from_disk(&self) -> bool {
        matches!(self, Self::TwoWay | Self::DiskToStudio)
    }
}

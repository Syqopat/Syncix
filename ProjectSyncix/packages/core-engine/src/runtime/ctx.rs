//! What every message handler is allowed to reach.
//!
//! The handlers used to be branches of one 1500-line function and simply closed over
//! its locals. As files, they need the same handful of things named once.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::model::SharedDataModel;
use crate::project::ProjectConfig;
use crate::server::AppState;
use crate::transport::StudioOutbox;

pub(crate) struct Ctx {
    pub state: Arc<AppState>,
    pub data_model: SharedDataModel,
    pub studio_outbox: Arc<StudioOutbox>,
    pub cfg: Arc<ProjectConfig>,
    /// Poked after every change so the writer wakes up.
    pub disk_notify: Arc<tokio::sync::Notify>,
    /// False until Studio has completed one full sync in this session. While it is
    /// false the writer may add files but must not delete any: reconciling an empty
    /// model against the disk once moved a whole sync folder to the trash.
    pub model_authoritative: Arc<AtomicBool>,
}

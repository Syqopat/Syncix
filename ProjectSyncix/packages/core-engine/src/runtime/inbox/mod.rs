//! The message loop: one file per kind of message Studio sends.

mod attributes;
mod composite;
mod create;
mod delete;
mod full_sync;
mod get_tree;
mod property;
mod rename;
mod reparent;
mod selection;
mod tags;

use tokio::sync::mpsc::Receiver;

use super::Ctx;
use crate::transport::{EventType, Payload};
use crate::{project, transport};

/// Reads messages from Studio until the channel closes.
pub(crate) async fn serve(ctx: Ctx, mut incoming: Receiver<Payload>) {
    let mut full_sync_parts = transport::FullSyncAssembler::new();

    while let Some(payload) = incoming.recv().await {
        // A big place's tree arrives in parts; nothing happens until the last one is in.
        let Some(payload) = full_sync_parts.accept(payload) else {
            continue;
        };

        // With the Studio -> disk direction off, nothing coming from Studio reaches the
        // model. FULL_SYNC is the exception: even in disk_to_studio mode the core has to
        // know Studio's UUIDs, or it cannot tell which object a file belongs to.
        if !ctx.cfg.mode_value.accepts_from_studio() && payload.event_type != EventType::FullSync {
            continue;
        }

        match payload.event_type {
            EventType::FullSync => full_sync::handle(&ctx, &payload).await,
            EventType::GetTree => get_tree::handle(&ctx, &payload).await,
            EventType::RenameInstance => rename::handle(&ctx, &payload).await,
            EventType::CreateInstance => create::handle(&ctx, &payload).await,
            EventType::DeleteInstance => delete::handle(&ctx, &payload).await,
            EventType::ReparentInstance => reparent::handle(&ctx, &payload).await,
            EventType::SetProperty => property::handle(&ctx, &payload).await,
            EventType::Selection => selection::handle(&ctx, &payload).await,
            EventType::SetTags => tags::handle(&ctx, &payload).await,
            EventType::SetAttribute => attributes::handle(&ctx, &payload).await,
            EventType::CompositeUpdate => composite::handle(&ctx, &payload).await,
            _ => {}
        }

        // Any of them may have changed the model, so the writer is poked once here
        // instead of in eleven places.
        ctx.disk_notify.notify_one();
    }

    let _ = project::is_sync_suspended();
}

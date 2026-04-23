pub mod doc_store_ref;
pub mod room;
pub mod snapshot;
pub mod sync_protocol;
pub mod title_sync;

pub use doc_store_ref::init_doc_store;
pub use room::{AwarenessPeer, DocRoom, DocStore, awareness_last_active_to_datetime};
pub use sync_protocol::{
    CollabMessage, apply_update_safe, decode_message, encode_full_sync_step2, encode_sync_step1,
    encode_sync_step2, encode_title_update, encode_update,
};

/// Shared event runtime — all crates in this binary re-export the same statics.
pub mod tokio_event_adapter_runtime {
    pub use utils::event_runtime::*;
}

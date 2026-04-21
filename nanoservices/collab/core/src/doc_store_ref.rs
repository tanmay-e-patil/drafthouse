use crate::DocStore;
use std::sync::{Arc, OnceLock};

static DOC_STORE: OnceLock<Arc<DocStore>> = OnceLock::new();

pub fn init_doc_store(store: Arc<DocStore>) {
    let _ = DOC_STORE.set(store);
}

pub fn get_doc_store() -> Option<&'static Arc<DocStore>> {
    DOC_STORE.get()
}

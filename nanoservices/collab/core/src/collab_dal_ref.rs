use dal::ScyllaDescriptor;
use std::sync::{Arc, OnceLock};

static COLLAB_DAL: OnceLock<Arc<ScyllaDescriptor>> = OnceLock::new();

pub fn init_collab_dal(dal: Arc<ScyllaDescriptor>) {
    let _ = COLLAB_DAL.set(dal);
}

pub fn get_collab_dal() -> Option<&'static Arc<ScyllaDescriptor>> {
    COLLAB_DAL.get()
}

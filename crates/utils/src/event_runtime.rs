use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;

type HandlerFn = fn(Vec<u8>) -> Pin<Box<dyn Future<Output = ()> + Send>>;

static HANDLERS: Mutex<Option<HashMap<String, HandlerFn>>> = Mutex::new(None);

pub fn insert_into_hashmap(type_name: String, handler: HandlerFn) {
    HANDLERS
        .lock()
        .unwrap()
        .get_or_insert_with(HashMap::new)
        .insert(type_name, handler);
}

pub fn publish_event(name: &str, data: Vec<u8>) {
    let handler = HANDLERS
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|m| m.get(name).copied());

    if let Some(f) = handler {
        tokio::spawn(f(data));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[tokio::test]
    async fn registered_handler_called_on_publish() {
        static CALLED: AtomicBool = AtomicBool::new(false);

        fn handler(_data: Vec<u8>) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            Box::pin(async move {
                CALLED.store(true, Ordering::SeqCst);
            })
        }

        insert_into_hashmap("TestEvent".to_string(), handler);
        publish_event("TestEvent", vec![]);

        tokio::task::yield_now().await;
        assert!(CALLED.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn unknown_event_does_not_panic() {
        // No handler registered for "UnknownEvent" — should silently do nothing
        publish_event("UnknownEvent", vec![]);
    }
}

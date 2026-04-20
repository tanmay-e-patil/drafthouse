# In-Process Event System: `subscribe_to_event` + `publish_event!`

Two proc-macro crates that wire up a type-driven, in-process pub/sub system using Tokio. No external broker. Events are Rust structs; routing is by type name; transport is `bincode`.

---

## Crates

| Crate | Cargo name | Kind |
|---|---|---|
| `crates/event-subscriber` | `nan-serve-event-subscriber` | `proc-macro = true` |
| `crates/publish-event` | `nan-serve-publish-event` | `proc-macro = true` |

Both depend only on `quote`, `syn`, and the standard proc-macro machinery. All runtime behaviour lives in a module the consuming crate must provide.

---

## Required Runtime Module

Both macros emit calls into `crate::tokio_event_adapter_runtime`. The consuming crate **must** expose this module with at least two functions:

```rust
// crate::tokio_event_adapter_runtime

/// Register a handler for a message type.
/// `type_name` is the bare struct name (last `::` segment), e.g. "AddNumbers".
/// `handler` is a function pointer: receives bincode bytes, returns a boxed future.
pub fn insert_into_hashmap(
    type_name: String,
    handler: fn(Vec<u8>) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>,
);

/// Dispatch an event.
/// `name` is the bare struct name. `data` is the bincode-serialised payload.
pub fn publish_event(name: &str, data: Vec<u8>);
```

A minimal implementation uses a `Mutex<HashMap<String, fn(Vec<u8>) -> ...>>` for registration and lookup, then `tokio::spawn` to run the handler.

### Minimal example

```rust
// src/tokio_event_adapter_runtime.rs

use std::collections::HashMap;
use std::pin::Pin;
use std::future::Future;
use std::sync::Mutex;

type HandlerFn = fn(Vec<u8>) -> Pin<Box<dyn Future<Output = ()> + Send>>;

static HANDLERS: Mutex<Option<HashMap<String, HandlerFn>>> = Mutex::new(None);

fn handlers() -> std::sync::MutexGuard<'static, Option<HashMap<String, HandlerFn>>> {
    HANDLERS.lock().unwrap()
}

pub fn insert_into_hashmap(type_name: String, handler: HandlerFn) {
    handlers().get_or_insert_with(HashMap::new).insert(type_name, handler);
}

pub fn publish_event(name: &str, data: Vec<u8>) {
    let handler = handlers()
        .as_ref()
        .and_then(|m| m.get(name).copied());

    if let Some(f) = handler {
        tokio::spawn(f(data));
    }
}
```

---

## `#[subscribe_to_event]`

### What it does

Attach to any `async fn` that takes exactly one argument. The argument type must implement `serde::Serialize + serde::de::DeserializeOwned`.

```rust
use nan_serve_event_subscriber::subscribe_to_event;

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct OrderPlaced {
    order_id: i64,
}

#[subscribe_to_event]
async fn handle_order(event: OrderPlaced) {
    println!("Processing order {}", event.order_id);
}
```

### Generated code (conceptual)

```rust
// Original function preserved unchanged
async fn handle_order(event: OrderPlaced) { ... }

// Compile-time trait check (fails at compile time if OrderPlaced is not Serialize+Deserialize)
fn _check_handle_order<T: serde::Serialize + serde::de::DeserializeOwned>() {}
const _: fn() = || { _check_handle_order::<OrderPlaced>(); };

// Bridge: raw bytes -> typed call
fn routed_handle_order(data: Vec<u8>) -> Pin<Box<dyn Future<Output=()> + Send>> {
    Box::pin(async move {
        let deserialized: OrderPlaced = nanoservices_utils::bincode::deserialize(&data).unwrap();
        handle_order(deserialized).await;
    })
}

// Registration
fn register_handle_order() {
    crate::tokio_event_adapter_runtime::insert_into_hashmap(
        "OrderPlaced".to_string(),   // bare type name, used as routing key
        routed_handle_order,
    );
}

// Auto-run at binary start via `ctor`
#[ctor::ctor]
fn init_handle_order() {
    println!("Initializing function: handle_order");
    register_handle_order();
}
```

### Routing key

The key is the **full type path string**, e.g. `"my_crate::events::OrderPlaced"`. It is derived from `param_type.to_token_stream().to_string()`. The publisher uses `std::any::type_name_of_val` which produces the same qualified path at runtime — they must match.

### Constraints enforced at compile time

- Function must be `async`.
- Function must have **exactly one** parameter.
- Parameter type must implement `Serialize + DeserializeOwned` (checked via a hidden generic function).

---

## `publish_event!(instance)`

### What it does

A function-like macro. Pass any value by identifier; the macro serialises it and dispatches to all registered handlers for that type.

```rust
use nan_serve_publish_event::publish_event;

let order = OrderPlaced { order_id: 42 };
publish_event!(order);
```

### Generated code (conceptual)

```rust
{
    let type_name = std::any::type_name_of_val(&order);
    let name = type_name.split("::").last().unwrap(); // "OrderPlaced"
    let data = bincode::serialize(&order).unwrap();
    crate::tokio_event_adapter_runtime::publish_event(name, data);
}
```

**Note:** uses `bincode` directly (not through `nanoservices_utils`) and calls `type_name_of_val` at runtime. The routing key is only the **last segment** of the path (`split("::").last()`), so two types with the same bare name in different modules would collide.

---

## Serialisation

- **Transport format:** `bincode` (compact binary, not self-describing).
- The subscriber deserialises with `nanoservices_utils::bincode::deserialize`.
- The publisher serialises with `bincode::serialize`.
- Both must agree on the same `bincode` version and the struct must be identical at both call sites.

---

## Dependency requirements in the consuming crate

```toml
[dependencies]
nan-serve-event-subscriber = { path = "..." }   # or crates.io version
nan-serve-publish-event    = { path = "..." }
nanoservices_utils         = "..."              # provides bincode + ctor re-exports
bincode                    = "1"
serde                      = { version = "1", features = ["derive"] }
tokio                      = { version = "1", features = ["full"] }
```

And the consuming crate's `lib.rs` / `main.rs` must `pub mod tokio_event_adapter_runtime;` at the crate root.

---

## End-to-end usage pattern

```rust
// src/tokio_event_adapter_runtime.rs  — implement as shown above
// src/events.rs

use serde::{Serialize, Deserialize};
use nan_serve_event_subscriber::subscribe_to_event;
use nan_serve_publish_event::publish_event;

#[derive(Serialize, Deserialize, Debug)]
pub struct UserSignedUp {
    pub user_id: i64,
    pub email: String,
}

// Register handler at startup automatically (via ctor)
#[subscribe_to_event]
async fn on_user_signed_up(event: UserSignedUp) {
    println!("Send welcome email to {}", event.email);
}

// From anywhere in the crate:
pub async fn register_user(user_id: i64, email: String) {
    // ... persist user ...

    let event = UserSignedUp { user_id, email };
    publish_event!(event);   // non-blocking: spawns a Tokio task
}
```

---

## Key design decisions (for reimplementation)

| Decision | Detail |
|---|---|
| Routing key | Bare type name (last `::` segment). Global uniqueness within the crate is the caller's responsibility. |
| Registration timing | `#[ctor]` — runs before `main`, so handlers are always registered before first publish. |
| Dispatch model | `tokio::spawn` — fire-and-forget; no return value, no backpressure. |
| Error handling | `unwrap()` on both serialise and deserialise — panics on malformed data. Production use should replace with error propagation. |
| Multiple handlers | `insert_into_hashmap` inserts by key; if multiple functions subscribe to the same type, the last registered wins unless the runtime uses a `Vec` per key instead of a single `HandlerFn`. |
| Cross-crate events | Not supported — `crate::tokio_event_adapter_runtime` is always the local crate. Each binary has its own registry. |

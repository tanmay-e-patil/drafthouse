use proc_macro::TokenStream;
use quote::quote;
use syn::{Expr, parse_macro_input};

/// Publishes an in-process event to all registered subscribers.
///
/// Serialises the value with `bincode`, derives the routing key from
/// `std::any::type_name_of_val` (bare struct name), and dispatches via
/// `crate::tokio_event_adapter_runtime::publish_event`. Fire-and-forget:
/// each handler is spawned as an independent Tokio task.
///
/// # Example
/// ```ignore
/// publish_event!(TitleUpdated { doc_id, title });
/// ```
#[proc_macro]
pub fn publish_event(input: TokenStream) -> TokenStream {
    let expr = parse_macro_input!(input as Expr);

    let expanded = quote! {
        {
            let __event = #expr;
            let __data = ::bincode::serialize(&__event)
                .expect("publish_event: serialize failed");
            let __type_name = ::std::any::type_name_of_val(&__event);
            let __name = __type_name.split("::").last().unwrap_or(__type_name);
            crate::tokio_event_adapter_runtime::publish_event(__name, __data);
        }
    };

    TokenStream::from(expanded)
}

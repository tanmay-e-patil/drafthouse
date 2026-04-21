use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{FnArg, ItemFn, PatType, parse_macro_input};

/// Registers an async handler for an in-process event.
///
/// Attach to any `async fn` taking exactly one parameter whose type implements
/// `serde::Serialize + serde::de::DeserializeOwned`. At binary startup (via
/// `#[ctor::ctor]`) the handler is inserted into the global registry in
/// `crate::tokio_event_adapter_runtime`. The routing key is the bare struct
/// name (last `::` segment of the type path).
///
/// # Example
/// ```ignore
/// #[subscribe_to_event]
/// async fn on_order_placed(event: OrderPlaced) { ... }
/// ```
#[proc_macro_attribute]
pub fn subscribe_to_event(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let func = parse_macro_input!(item as ItemFn);
    let func_name = &func.sig.ident;

    let param = func
        .sig
        .inputs
        .first()
        .expect("subscribe_to_event: function must have exactly one parameter");

    let param_type = match param {
        FnArg::Typed(PatType { ty, .. }) => ty,
        _ => panic!("subscribe_to_event: unexpected self parameter"),
    };

    // Bare type name used as routing key (matches publisher's split("::").last())
    let type_str = quote!(#param_type).to_string();
    let bare_name = type_str
        .split("::")
        .last()
        .unwrap_or(&type_str)
        .trim()
        .to_string();

    let routed_fn = syn::Ident::new(&format!("__routed_{func_name}"), Span::call_site());
    let register_fn = syn::Ident::new(&format!("__register_{func_name}"), Span::call_site());
    let init_fn = syn::Ident::new(&format!("__init_{func_name}"), Span::call_site());

    let expanded = quote! {
        #func

        fn #routed_fn(
            data: Vec<u8>,
        ) -> ::std::pin::Pin<Box<dyn ::std::future::Future<Output = ()> + Send>> {
            Box::pin(async move {
                let event: #param_type =
                    ::bincode::deserialize(&data).expect("subscribe_to_event: deserialize failed");
                #func_name(event).await;
            })
        }

        fn #register_fn() {
            crate::tokio_event_adapter_runtime::insert_into_hashmap(
                #bare_name.to_string(),
                #routed_fn,
            );
        }

        #[::ctor::ctor]
        fn #init_fn() {
            #register_fn();
        }
    };

    TokenStream::from(expanded)
}

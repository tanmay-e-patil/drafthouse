use proc_macro::TokenStream;

/// Stub: will implement `#[impl_transaction]` in a later slice.
#[proc_macro_attribute]
pub fn impl_transaction(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

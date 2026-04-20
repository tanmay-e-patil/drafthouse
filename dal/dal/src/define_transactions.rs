#[macro_export]
macro_rules! define_dal_transactions {
    (
        $( $trait:ident => $func_name:ident ($($param:ident : $ptype:ty),*) -> $rtype:ty ),* $(,)?
    ) => {
        $(
            pub trait $trait {
                fn $func_name(&self, $($param : $ptype),*)
                    -> impl std::future::Future<Output = Result<$rtype, utils::errors::NanoServiceError>> + Send;
            }
        )*
    };
}

# Architecture Quick Start Guide

This guide is a condensed version of the codebase architecture for rapid onboarding.

## 1. The Three Layers
Every feature is split across three crates to ensure isolation and testability.

| Layer | Crate | Responsibility | Knows About... |
| :--- | :--- | :--- | :--- |
| **Networking** | `nanoservices/<name>/networking` | HTTP handlers, JSON parsing, Route registration. | Actix-web, Core, Kernel, DAL, Utils |
| **Core** | `nanoservices/<name>/core` | Business logic, validation, orchestration. | Kernel, Utils, DAL |
| **DAL** | `dal/dal` | SQL queries, Database interactions. | Kernel, SQLx, Utils |

**The Kernel (`dal/kernel`)** is the shared "vocabulary". It contains all data models (Structs). The interface definitions (Traits) are defined in the `DAL` crate (e.g., `dal/dal/src/*/tx_definitions.rs`) using the `define_dal_transactions!` macro.

---

## 2. Dependency Injection via Generics
We don't use DI containers. We use **Trait Bounds** on generic type parameters (`<X, Y, Z>`).

```rust
// In Core: I need "something" (X) that can create a user.
pub async fn create_user<X: CreateUser>(new_user: NewUser) -> Result<User, NanoServiceError> {
    X::create_user(new_user).await
}
```

The **Concrete Implementation** (e.g., `SqlxPostGresDescriptor`) is only injected once, at the **Route Registration** level in the Networking layer.

---

## 3. Workflow: Adding a New Feature
Follow these 5 steps to add a new API endpoint (e.g., "Get Todo By ID"):

1.  **Define Model & Trait:**
    *   Add the data model to `dal/kernel/src/to_do_items.rs`.
    *   Add the trait definition in the DAL crate (e.g., `dal/dal/src/to_do_items/tx_definitions.rs`) using the `define_dal_transactions!` macro.
2.  **Implement in DAL:**
    *   In `dal/dal/src/to_do_items/postgres_txs.rs`, implement the trait for `SqlxPostGresDescriptor` using the `#[impl_transaction]` macro.
3.  **Write Logic in Core:**
    *   Create `nanoservices/to_do/core/src/api/get_by_id.rs`.
    *   Write a generic function: `pub async fn get_todo<X: GetToDoItem>(id: i32) -> ...`.
4.  **Create Handler in Networking:**
    *   Create `nanoservices/to_do/networking/src/api/get_by_id.rs`.
    *   Write the Actix-web handler using `#[api_endpoint]`.
5.  **Register Route:**
    *   In `nanoservices/to_do/networking/src/api/mod.rs`, add the route to the `views_factory` and inject `SqlxPostGresDescriptor`.

---

## 4. Essential Macros

### `#[api_endpoint]` (Networking)
Wraps a handler to automatically handle:
*   JWT Extraction & Validation.
*   Role-based access control (RBAC).
*   Session cache checks.
*   Standardized `Result<HttpResponse, NanoServiceError>` return type.

### `#[impl_transaction]` (DAL)
Reduces boilerplate when implementing a DAL trait for the Postgres descriptor. It is equally crucial for effortlessly generating mock DB implementations (e.g. `MockDb`) for testing core logic in isolation.

Usage example:
```rust
#[impl_transaction(SqlxPostGresDescriptor, CreateUser, create_user)]
async fn create_user(user: NewUser) -> Result<User, NanoServiceError> {
    // SQL logic here
}
```

Implementation (`crates/dal-tx-impl/src/lib.rs`):
```rust
extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, parse::Parse, parse::ParseStream,
    ItemFn, Ident, Token, Result
};

struct ImplementTraitArgs {
    struct_name: Ident,
    trait_name: Ident,
    fn_name: Ident,
}

impl Parse for ImplementTraitArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let struct_name: Ident = input.parse()?;
        input.parse::<Token![,]>()?;
        let trait_name: Ident = input.parse()?;
        input.parse::<Token![,]>()?;
        let fn_name: Ident = input.parse()?;
        Ok(Self { struct_name, trait_name, fn_name })
    }
}

#[proc_macro_attribute]
pub fn impl_transaction(attr: TokenStream, item: TokenStream) -> TokenStream {
    let ImplementTraitArgs { struct_name, trait_name, fn_name } = parse_macro_input!(attr as ImplementTraitArgs);
    let input_fn = parse_macro_input!(item as ItemFn);

    let fn_inputs = &input_fn.sig.inputs;
    let fn_body = &input_fn.block;
    let fn_generics = &input_fn.sig.generics;
    
    let fn_output = match &input_fn.sig.output {
        syn::ReturnType::Type(_, ty) => ty.as_ref(),
        syn::ReturnType::Default => panic!("Function must have a return type.")
    };

    let expanded = quote! {
        impl #trait_name for #struct_name {
            fn #fn_name #fn_generics (#fn_inputs) -> impl std::future::Future<Output = #fn_output> + Send {
                async move #fn_body
            }
        }
    };
    TokenStream::from(expanded)
}
```

---

## 5. Cheat Sheet: Where is it?

*   **Error Types:** `crates/utils/src/errors.rs`
*   **Database connection pool:** `dal/dal/src/connections/sqlx_postgres.rs`
*   **JWT logic:** `dal/kernel/src/token/`
*   **Global Entry Point:** `ingress/src/main.rs`
*   **Env Config:** `crates/utils/src/config.rs`

---

## 6. Other Key Macros

### `define_dal_transactions!` (DAL)
Used to generate the Kernel interface traits (e.g., `CreateUser`, `GetUser`). Instead of writing out the full trait definition, you pass it a simplified DSL.

Usage example (`dal/dal/src/to_do_items/tx_definitions.rs`):
```rust
define_dal_transactions!(
    CreateUser => create_user(user: NewUser) -> i32,
    DeleteUser => delete_user(id: i32) -> bool
);
```

Implementation (`dal/dal/src/define_transactions.rs`):
```rust
#[macro_export]
macro_rules! define_dal_transactions {
    (
        $( $trait:ident => $func_name:ident $(< $($generic:tt),* >)? ($($param:ident : $ptype:ty),*) -> $rtype:ty ),* $(,)?
    ) => {
        $(
            pub trait $trait {
                fn $func_name $(< $($generic),* >)? ($($param : $ptype),*) -> impl std::future::Future<Output = Result<$rtype, utils::errors::NanoServiceError>> + Send;
            }
        )*
    };
}
```

### `safe_eject!` (Utils)
Standardizes error handling. It takes a `Result`, and if it's an `Err`, it converts it into the workspace's universal `NanoServiceError`. Use the `?` operator after it to propagate the error.

Usage example:
```rust
let user = safe_eject!(
    sqlx::query_as!(...).fetch_one(&pool).await,
    NanoServiceErrorStatus::NotFound,
    "User not found"
)?;
```

Implementation (`crates/utils/src/errors.rs`):
```rust
#[macro_export]
macro_rules! safe_eject {
    ($e:expr, $err_status:expr) => {
        $e.map_err(|x| NanoServiceError::new(x.to_string(), $err_status))
    };
    ($e:expr, $err_status:expr, $message_context:expr) => {
        $e.map_err(|x| NanoServiceError::new(
                format!("{}: {}", $message_context, x.to_string()),
                $err_status
            )
        )
    };
}
```

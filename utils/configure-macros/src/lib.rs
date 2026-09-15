#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

/// Declares that a type is the contents of a configuration file.
///
/// The trait's methods have default bodies, so this adds nothing but the declaration:
/// the point of a derive here is that being a configuration is *opt-in*, rather than a
/// blanket implementation that would quietly cover every type serde can handle.
///
/// The type must also be serializable both ways — `Serialize` and `Deserialize` — for
/// the implementation to be accepted; if it is not, the missing bound is what the
/// compiler names.
///
/// ```rust
/// use rorolala_utils_configure::Configure;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Serialize, Deserialize, Configure)]
/// struct VaultConfig {
///     name: String,
/// }
///
/// fn takes_a_configuration<C: Configure>() {}
/// takes_a_configuration::<VaultConfig>();
/// ```
#[proc_macro_derive(Configure)]
pub fn derive_configure(input: TokenStream) -> TokenStream {
    let item = parse_macro_input!(input as DeriveInput);
    let name = &item.ident;
    let (impl_generics, type_generics, where_clause) = item.generics.split_for_impl();

    quote! {
        impl #impl_generics ::rorolala_utils_configure::Configure for #name #type_generics
        #where_clause
        {
        }
    }
    .into()
}

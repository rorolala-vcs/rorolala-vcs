#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

use proc_macro::TokenStream;
use proc_macro2::{Literal, Span, TokenStream as TokenStream2};
use quote::{ToTokens, format_ident, quote};
use rorolala_utils_lazyffi_core::{
    PAYLOAD_UNIT_FIELD, VariantFields, method_name, payload_type_name, tag_type_name, type_name,
    value_name, variant_needs_companion, variant_type_name,
};
use syn::{
    Attribute, Expr, ExprLit, ExprPath, Fields, FnArg, Ident, ImplItem, Item, ItemConst, ItemEnum,
    ItemFn, ItemImpl, ItemStruct, Lit, Meta, Pat, Receiver, ReceiverKind, ReturnType, Signature,
    Token, Type, Variant,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
};

/// Renders the scalar types handed over by `for_each_scalar!` as a name array.
macro_rules! scalar_names {
    ($($scalar:ident),* $(,)?) => {
        &[$(stringify!($scalar)),*]
    };
}

/// Scalar types that can be exported as a `static`.
///
/// Taken from `lazyffi-core`, so the set cannot drift from the conversions
/// `builtin` actually implements.
const SCALAR_TYPES: &[&str] = rorolala_utils_lazyffi_core::for_each_scalar!(scalar_names);

/// Safety section appended to every generated `extern "C"` wrapper.
const SAFETY_DOC: &str = "# Safety\n\nEvery pointer argument must be valid, aligned and readable \
     (or writable, for `&mut` parameters) for the duration of the call, and must uphold whatever \
     the corresponding conversion requires.";

/// Arguments accepted by the [`lazyffi`] attribute.
#[derive(Debug, Default)]
struct LazyFfiArgs {
    /// Explicit name for the generated FFI item.
    export: Option<Ident>,
}

impl Parse for LazyFfiArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut args = Self::default();

        if input.is_empty() {
            return Ok(args);
        }

        for meta in Punctuated::<Meta, Token![,]>::parse_terminated(input)? {
            let Meta::NameValue(name_value) = &meta else {
                return Err(syn::Error::new_spanned(
                    &meta,
                    "expected a `name = value` argument, e.g. `export = FFIMyType`",
                ));
            };

            if !name_value.path.is_ident("export") {
                return Err(syn::Error::new_spanned(
                    &name_value.path,
                    "unknown `lazyffi` argument; the only supported one is `export`",
                ));
            }

            args.export = Some(export_name(&name_value.value)?);
        }

        Ok(args)
    }
}

/// Reads the value of `export = ...`, accepting a bare identifier or a string literal.
fn export_name(expr: &Expr) -> syn::Result<Ident> {
    if let Expr::Path(ExprPath { path, .. }) = expr
        && let Some(ident) = path.get_ident()
    {
        return Ok(ident.clone());
    }

    if let Expr::Lit(ExprLit {
        lit: Lit::Str(lit_str),
        ..
    }) = expr
    {
        return lit_str.parse();
    }

    Err(syn::Error::new_spanned(
        expr,
        "expected a bare identifier (`export = FFIMyType`) or a string literal",
    ))
}

/// Export a Rust item to FFI.
///
/// # Arguments
///
/// - `export = <Name>` — the name of the generated FFI item. Accepts a bare
///   identifier (`export = FFIMyType`) or a string literal (`export = "FFIMyType"`).
///   When omitted the name derives from the Rust item: `ffi_<snake_case>` for
///   value-level items (`fn`, `const`), `FFI<PascalCase>` for types (`struct`,
///   `enum`).
///
/// # Supported items
///
/// | Item | Behaviour |
/// | --- | --- |
/// | `const` | Supported: `&str`, `char`, `bool` and numeric primitives. |
/// | `struct` | Supported: named fields, no generics. |
/// | `enum` | Supported: unit-only becomes a `#[repr(C)]` enum, variants with data become a tag plus a payload union. |
/// | `fn` | Supported: by-value and `&mut` parameters, by-value return. |
/// | `impl` | Supported: inherent impls, flattened into free functions whose first parameter is the receiver. |
///
/// Every Rust type has a single repr-C sibling named by `export`; all four
/// conversion traits point at it. The original item is always kept and the FFI
/// item is generated next to it:
///
/// - a `const` scalar becomes a `#[unsafe(no_mangle)] static`, a `const` `&str`
///   becomes an `extern "C"` function allocating a C string (release it with
///   `rorolala_utils_lazyffi::ffi_free_string`),
/// - a `struct` gains a `#[repr(C)]` sibling plus the four conversions,
/// - a unit-only `enum` gains a `#[repr(C)]` sibling plus the four conversions,
/// - an `enum` with data gains a repr struct holding a generated `<repr>Tag` enum
///   and a `<repr>Payload` union, plus the four conversions,
/// - a `fn` gains one `unsafe extern "C"` wrapper,
/// - an `impl` gains one such wrapper per method, as an associated function of
///   the same impl.
///
/// # Naming
///
/// Defaults come from `rorolala-utils-lazyffi-core`: `ffi_<snake_case>` for
/// `fn`/`const`, `ffi_<snake_case(type)>_<snake_case(method)>` for methods, and
/// `FFI<PascalCase>` for `struct`/`enum` (with `Tag`, `Payload` and
/// `<repr><Variant>` derivatives for the parts of a data-carrying enum). The
/// repr numbers enum variants from zero in declaration order; the Rust
/// discriminants are not mirrored, because the conversions match on the variant
/// rather than reinterpreting the value.
///
/// # Documentation
///
/// Rust docs stay on the Rust side, except for two parts that are carried into
/// the generated C header:
///
/// - the **first line**, which is the summary;
/// - the section under a **`# FFI`** heading, which is written for C callers. The
///   heading itself is dropped and the section ends at the next heading, so
///   `# Safety`, `# Panics` and anything else remain Rust-only.
///
/// ```rust,ignore
/// /// A rectangle.
/// ///
/// /// # FFI
/// /// Passed by value; the fields are copied, not shared.
/// #[lazyffi]
/// pub struct Rect { /* ... */ }
/// ```
///
/// # References
///
/// `&mut T` parameters are supported: the value is copied in, the function runs
/// with `&mut`, and the result is written back through the same pointer. Shared
/// `&T` parameters are rejected — a shared reference would let the C side mutate
/// memory Rust treats as immutable. Reference returns are rejected too: the
/// wrapper cannot guarantee the referent outlives the call.
#[proc_macro_attribute]
pub fn lazyffi(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as LazyFfiArgs);
    let item = parse_macro_input!(item as Item);

    match expand(&args, &item) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.into_compile_error().into(),
    }
}

/// Dispatches on the annotated item.
fn expand(args: &LazyFfiArgs, item: &Item) -> syn::Result<TokenStream2> {
    match item {
        Item::Const(konst) => expand_const(args, konst),
        Item::Enum(item) => expand_enum(args, item),
        Item::Impl(item) => expand_impl(args, item),
        Item::Struct(item) => expand_struct(args, item),
        Item::Fn(item) => expand_fn(args, item),
        other => Err(syn::Error::new_spanned(
            other,
            "`#[lazyffi]` can be applied to `const`, `enum`, `struct`, `fn` or `impl` items",
        )),
    }
}

/// Default FFI name for a value-level item (`fn`, `const`), from `lazyffi-core`.
fn default_value_name(rust_name: &Ident) -> Ident {
    Ident::new(&value_name(&rust_name.to_string()), rust_name.span())
}

/// Default FFI name for a type (`struct`, `enum`), from `lazyffi-core`.
fn default_type_name(rust_name: &Ident) -> Ident {
    Ident::new(&type_name(&rust_name.to_string()), rust_name.span())
}

/// Documentation carried over to a generated item, falling back to a generated
/// sentence when the original has none, so `#[lazyffi]` stays usable in crates
/// that deny `missing_docs`.
fn docs_or_fallback(what: &str, attrs: &[Attribute]) -> Vec<TokenStream2> {
    let docs: Vec<TokenStream2> = attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"))
        .map(ToTokens::to_token_stream)
        .collect();

    if docs.is_empty() {
        let fallback = format!("FFI export of {what}.");
        vec![quote! { #[doc = #fallback] }]
    } else {
        docs
    }
}

/// Attributes mirrored onto a generated item: `doc` (with a fallback) plus
/// `cfg`, so an export is documented and conditionally compiled like the item it
/// mirrors.
fn mirrored_attrs(what: &str, attrs: &[Attribute]) -> Vec<TokenStream2> {
    let mut mirrored = docs_or_fallback(what, attrs);
    mirrored.extend(
        attrs
            .iter()
            .filter(|attr| attr.path().is_ident("cfg"))
            .map(ToTokens::to_token_stream),
    );
    mirrored
}

/// Expands `#[lazyffi]` on a `const` item.
fn expand_const(args: &LazyFfiArgs, konst: &ItemConst) -> syn::Result<TokenStream2> {
    let rust_name = &konst.ident;
    let ffi_name = args
        .export
        .clone()
        .unwrap_or_else(|| default_value_name(rust_name));
    let ty = &konst.ty;
    let mirrored = mirrored_attrs(&format!("`{rust_name}`"), &konst.attrs);

    if is_str_ref(ty) {
        return Ok(quote! {
            #konst

            #(#mirrored)*
            #[unsafe(no_mangle)]
            #[allow(nonstandard_style)]
            pub extern "C" fn #ffi_name() -> *mut ::core::ffi::c_char {
                ::rorolala_utils_lazyffi::__export_str(#rust_name)
            }
        });
    }

    if is_scalar(ty) {
        return Ok(quote! {
            #konst

            #(#mirrored)*
            #[unsafe(no_mangle)]
            #[allow(nonstandard_style)]
            pub static #ffi_name: #ty = #rust_name;
        });
    }

    Err(syn::Error::new_spanned(
        ty,
        "`#[lazyffi]` on `const` only supports `&str`, `char`, `bool` and numeric primitives",
    ))
}

/// Expands `#[lazyffi]` on a `struct` item.
fn expand_struct(args: &LazyFfiArgs, item: &ItemStruct) -> syn::Result<TokenStream2> {
    if !item.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &item.generics,
            "`#[lazyffi]` does not support generic types",
        ));
    }

    let rust_name = &item.ident;
    let ffi_name = args
        .export
        .clone()
        .unwrap_or_else(|| default_type_name(rust_name));

    let Fields::Named(named) = &item.fields else {
        return Err(syn::Error::new_spanned(
            &item.fields,
            "`#[lazyffi]` requires a struct with named fields",
        ));
    };

    let mut ffi_fields = Vec::new();
    let mut from_ffi = Vec::new();
    let mut to_ffi = Vec::new();

    for field in &named.named {
        let Some(field_name) = field.ident.as_ref() else {
            continue;
        };

        if field.attrs.iter().any(|attr| attr.path().is_ident("cfg")) {
            return Err(syn::Error::new_spanned(
                field,
                "`#[lazyffi]` does not support `cfg` on struct fields yet",
            ));
        }

        let field_ty = &field.ty;
        let field_docs = docs_or_fallback(&format!("field `{field_name}`"), &field.attrs);

        ffi_fields.push(quote! {
            #(#field_docs)*
            pub #field_name: <#field_ty as ::rorolala_utils_lazyffi::ReturnType>::Target,
        });
        from_ffi.push(quote! {
            #field_name: unsafe {
                <#field_ty as ::rorolala_utils_lazyffi::InputType>::input_type(input.#field_name)
            },
        });
        to_ffi.push(quote! {
            #field_name: <#field_ty as ::rorolala_utils_lazyffi::ReturnType>::return_self(
                self.#field_name,
            ),
        });
    }

    let struct_docs = mirrored_attrs(&format!("`{rust_name}`"), &item.attrs);
    let conversions = conversion_impls(
        rust_name,
        &ffi_name,
        &quote! { Self { #(#from_ffi)* } },
        &quote! { #ffi_name { #(#to_ffi)* } },
    );

    Ok(quote! {
        #item

        #(#struct_docs)*
        #[repr(C)]
        #[derive(Clone, Copy)]
        #[allow(nonstandard_style)]
        pub struct #ffi_name {
            #(#ffi_fields)*
        }

        #conversions
    })
}

/// Expands `#[lazyffi]` on an `enum` item.
fn expand_enum(args: &LazyFfiArgs, item: &ItemEnum) -> syn::Result<TokenStream2> {
    if !item.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &item.generics,
            "`#[lazyffi]` does not support generic types",
        ));
    }

    for variant in &item.variants {
        if variant.attrs.iter().any(|attr| attr.path().is_ident("cfg")) {
            return Err(syn::Error::new_spanned(
                variant,
                "`#[lazyffi]` does not support `cfg` on enum variants yet",
            ));
        }
    }

    let rust_name = &item.ident;
    let ffi_name = args
        .export
        .clone()
        .unwrap_or_else(|| default_type_name(rust_name));

    let carries_data = item
        .variants
        .iter()
        .any(|variant| !matches!(variant.fields, Fields::Unit));

    let expansion = if carries_data {
        expand_data_enum(item, rust_name, &ffi_name)
    } else {
        expand_unit_enum(item, rust_name, &ffi_name)
    };

    Ok(expansion)
}

/// Maps a variant's fields onto the shape `lazyffi-core` reasons about.
fn variant_fields(fields: &Fields) -> VariantFields {
    match fields {
        Fields::Unit => VariantFields::Unit,
        Fields::Unnamed(unnamed) => VariantFields::Unnamed(unnamed.unnamed.len()),
        Fields::Named(named) => VariantFields::Named(named.named.len()),
    }
}

/// Expands a unit-only `enum` into a `#[repr(C)]` enum.
fn expand_unit_enum(item: &ItemEnum, rust_name: &Ident, ffi_name: &Ident) -> TokenStream2 {
    let mut ffi_variants = Vec::new();
    let mut from_ffi = Vec::new();
    let mut to_ffi = Vec::new();

    for (index, variant) in item.variants.iter().enumerate() {
        let name = &variant.ident;
        let docs = docs_or_fallback(&format!("variant `{name}`"), &variant.attrs);
        // The repr numbers the variants itself: the conversions match on the
        // variant, so the Rust discriminant never has to be mirrored.
        let value = Literal::usize_unsuffixed(index);

        ffi_variants.push(quote! {
            #(#docs)*
            #name = #value
        });
        from_ffi.push(quote! { #ffi_name::#name => Self::#name, });
        to_ffi.push(quote! { Self::#name => #ffi_name::#name, });
    }

    let docs = mirrored_attrs(&format!("`{rust_name}`"), &item.attrs);
    let conversions = conversion_impls(
        rust_name,
        ffi_name,
        &quote! { match input { #(#from_ffi)* } },
        &quote! { match self { #(#to_ffi)* } },
    );

    quote! {
        #item

        #(#docs)*
        #[repr(C)]
        #[derive(Clone, Copy)]
        #[allow(nonstandard_style, clippy::enum_variant_names)]
        pub enum #ffi_name {
            #(#ffi_variants),*
        }

        #conversions
    }
}

/// The name and type of one field of a variant's payload.
struct PayloadField {
    /// Identifier the field is bound to in generated code (`_0` for tuple fields).
    binding: Ident,
    /// The field's Rust type.
    ty: Type,
    /// Documentation mirrored onto the generated field.
    docs: Vec<TokenStream2>,
}

/// Names the fields of a variant's payload, so tuple and braced variants can be
/// generated by the same code.
fn payload_fields(fields: &Fields) -> Vec<PayloadField> {
    match fields {
        Fields::Unit => Vec::new(),
        Fields::Unnamed(unnamed) => unnamed
            .unnamed
            .iter()
            .enumerate()
            .map(|(index, field)| PayloadField {
                binding: format_ident!("_{index}"),
                ty: field.ty.clone(),
                docs: docs_or_fallback(&format!("field {index}"), &field.attrs),
            })
            .collect(),
        Fields::Named(named) => named
            .named
            .iter()
            .filter_map(|field| {
                let name = field.ident.clone()?;
                Some(PayloadField {
                    binding: name.clone(),
                    ty: field.ty.clone(),
                    docs: docs_or_fallback(&format!("field `{name}`"), &field.attrs),
                })
            })
            .collect(),
    }
}

/// Generated documentation plus the `cfg`s mirrored from `attrs`.
fn generated_attrs(doc: &str, attrs: &[Attribute]) -> Vec<TokenStream2> {
    let mut generated = vec![quote! { #[doc = #doc] }];
    generated.extend(
        attrs
            .iter()
            .filter(|attr| attr.path().is_ident("cfg"))
            .map(ToTokens::to_token_stream),
    );
    generated
}

/// The generated pieces of one variant of a data-carrying enum.
struct VariantParts {
    /// Companion struct definition, empty when the payload is a single field.
    companion: TokenStream2,
    /// One variant of the generated tag enum.
    tag_variant: TokenStream2,
    /// One field of the generated payload union.
    union_field: TokenStream2,
    /// Arm rebuilding the Rust variant from the repr.
    from_ffi: TokenStream2,
    /// Arm rebuilding the repr from the Rust variant.
    to_ffi: TokenStream2,
}

/// Expands an `enum` whose variants carry data into a tag plus a payload union.
fn expand_data_enum(item: &ItemEnum, rust_name: &Ident, ffi_name: &Ident) -> TokenStream2 {
    let repr = ffi_name.to_string();
    let tag_name = Ident::new(&tag_type_name(&repr), ffi_name.span());
    let payload_name = Ident::new(&payload_type_name(&repr), ffi_name.span());
    let unit_field = Ident::new(PAYLOAD_UNIT_FIELD, Span::call_site());

    let parts = item
        .variants
        .iter()
        .enumerate()
        .map(|(index, variant)| {
            expand_data_variant(
                variant,
                index,
                &repr,
                ffi_name,
                &tag_name,
                &payload_name,
                &unit_field,
            )
        })
        .collect::<Vec<_>>();

    // A union is built through one of its fields, and a unit variant has no
    // payload to name, so those variants share a zero-sized slot.
    let mut union_fields: Vec<TokenStream2> = Vec::new();
    if item
        .variants
        .iter()
        .any(|variant| matches!(variant.fields, Fields::Unit))
    {
        union_fields.push(quote! {
            #[doc = "Slot the variants without a payload are written through."]
            pub #unit_field: (),
        });
    }
    union_fields.extend(parts.iter().map(|part| part.union_field.clone()));

    let companions = parts.iter().map(|part| &part.companion);
    let tag_variants = parts.iter().map(|part| &part.tag_variant);
    let from_ffi = parts.iter().map(|part| &part.from_ffi);
    let to_ffi = parts.iter().map(|part| &part.to_ffi);

    let docs = mirrored_attrs(&format!("`{rust_name}`"), &item.attrs);
    let tag_docs = generated_attrs(
        &format!("Tag of the `#[lazyffi]` repr of `{rust_name}`."),
        &item.attrs,
    );
    let payload_docs = generated_attrs(
        &format!("Payload of the `#[lazyffi]` repr of `{rust_name}`."),
        &item.attrs,
    );
    let conversions = conversion_impls(
        rust_name,
        ffi_name,
        &quote! { match input.tag { #(#from_ffi)* } },
        &quote! { match self { #(#to_ffi)* } },
    );

    quote! {
        #item

        #(#companions)*

        #(#tag_docs)*
        #[repr(C)]
        #[derive(Clone, Copy)]
        #[allow(nonstandard_style, clippy::enum_variant_names)]
        pub enum #tag_name {
            #(#tag_variants),*
        }

        #(#payload_docs)*
        #[repr(C)]
        #[derive(Clone, Copy)]
        #[allow(nonstandard_style, clippy::pub_underscore_fields)]
        pub union #payload_name {
            #(#union_fields)*
        }

        #(#docs)*
        #[repr(C)]
        #[derive(Clone, Copy)]
        pub struct #ffi_name {
            /// The variant the payload belongs to.
            pub tag: #tag_name,
            /// The payload itself, meaningful only for the variant in `tag`.
            pub payload: #payload_name,
        }

        #conversions
    }
}

/// Builds the generated pieces of one variant of a data-carrying enum.
fn expand_data_variant(
    variant: &Variant,
    index: usize,
    repr: &str,
    ffi_name: &Ident,
    tag_name: &Ident,
    payload_name: &Ident,
    unit_field: &Ident,
) -> VariantParts {
    let variant_name = &variant.ident;
    let tag_docs = docs_or_fallback(&format!("variant `{variant_name}`"), &variant.attrs);
    let value = Literal::usize_unsuffixed(index);
    let tag_variant = quote! {
        #(#tag_docs)*
        #variant_name = #value
    };

    if matches!(variant.fields, Fields::Unit) {
        return VariantParts {
            companion: TokenStream2::new(),
            tag_variant,
            union_field: TokenStream2::new(),
            from_ffi: quote! { #tag_name::#variant_name => Self::#variant_name, },
            to_ffi: quote! {
                Self::#variant_name => #ffi_name {
                    tag: #tag_name::#variant_name,
                    payload: #payload_name { #unit_field: () },
                },
            },
        };
    }

    let fields = payload_fields(&variant.fields);
    let named = matches!(variant.fields, Fields::Named(_));
    let wrapped = variant_needs_companion(&variant_fields(&variant.fields));

    // A wrapped payload needs a companion struct; a single tuple field is used
    // directly as the union field's type.
    let companion = wrapped.then(|| {
        Ident::new(
            &variant_type_name(repr, &variant_name.to_string()),
            variant_name.span(),
        )
    });

    let (companion_definition, union_ty) =
        payload_repr(companion.as_ref(), &fields, variant_name, &variant.attrs);

    let payload_docs = docs_or_fallback(&format!("payload of `{variant_name}`"), &variant.attrs);
    let union_field = quote! {
        #(#payload_docs)*
        pub #variant_name: #union_ty,
    };

    // Reading a union field is the only unsafe step; everything around it is
    // ordinary value conversion. The pieces are collected rather than left as
    // iterators, because each is spliced into more than one arm.
    let bindings: Vec<&Ident> = fields.iter().map(|field| &field.binding).collect();
    let reads: Vec<TokenStream2> = fields
        .iter()
        .map(|PayloadField { binding, ty, .. }| {
            let access = if wrapped {
                quote! { input.payload.#variant_name.#binding }
            } else {
                quote! { input.payload.#variant_name }
            };
            quote! {
                unsafe { <#ty as ::rorolala_utils_lazyffi::InputType>::input_type(#access) }
            }
        })
        .collect();

    let payload_value = payload_value(companion.as_ref(), &fields);

    let constructed = if named {
        quote! { Self::#variant_name { #(#bindings: #reads,)* } }
    } else {
        quote! { Self::#variant_name(#(#reads),*) }
    };
    let pattern = if named {
        quote! { Self::#variant_name { #(#bindings),* } }
    } else {
        quote! { Self::#variant_name(#(#bindings),*) }
    };

    VariantParts {
        companion: companion_definition,
        tag_variant,
        union_field,
        from_ffi: quote! { #tag_name::#variant_name => #constructed, },
        to_ffi: quote! {
            #pattern => #ffi_name {
                tag: #tag_name::#variant_name,
                payload: #payload_name { #variant_name: #payload_value },
            },
        },
    }
}

/// The union field type of a variant's payload, plus the companion struct it
/// needs when the payload does not fit in a single field.
fn payload_repr(
    companion: Option<&Ident>,
    fields: &[PayloadField],
    variant_name: &Ident,
    attrs: &[Attribute],
) -> (TokenStream2, TokenStream2) {
    let Some(companion) = companion else {
        let ty = &first_field(fields).ty;
        return (
            TokenStream2::new(),
            quote! { <#ty as ::rorolala_utils_lazyffi::ReturnType>::Target },
        );
    };

    let companion_fields = fields.iter().map(|field| {
        let binding = &field.binding;
        let ty = &field.ty;
        let docs = &field.docs;
        quote! {
            #(#docs)*
            pub #binding: <#ty as ::rorolala_utils_lazyffi::ReturnType>::Target,
        }
    });
    let companion_docs =
        docs_or_fallback(&format!("payload of the `{variant_name}` variant"), attrs);

    (
        quote! {
            #(#companion_docs)*
            #[repr(C)]
            #[derive(Clone, Copy)]
            #[allow(
                nonstandard_style,
                clippy::pub_underscore_fields,
                clippy::struct_field_names
            )]
            pub struct #companion {
                #(#companion_fields)*
            }
        },
        quote! { #companion },
    )
}

/// Rebuilds a variant's payload from its bound fields, writing it into the union.
fn payload_value(companion: Option<&Ident>, fields: &[PayloadField]) -> TokenStream2 {
    let Some(companion) = companion else {
        let field = first_field(fields);
        let (binding, ty) = (&field.binding, &field.ty);
        return quote! { <#ty as ::rorolala_utils_lazyffi::ReturnType>::return_self(#binding) };
    };

    let writes = fields.iter().map(|PayloadField { binding, ty, .. }| {
        quote! {
            #binding: <#ty as ::rorolala_utils_lazyffi::ReturnType>::return_self(#binding),
        }
    });

    quote! { #companion { #(#writes)* } }
}

/// The single field of a payload that is not wrapped in a companion struct.
const fn first_field(fields: &[PayloadField]) -> &PayloadField {
    match fields.first() {
        Some(field) => field,
        None => panic!("a payload has at least one field"),
    }
}

/// Emits the four conversion traits for a type whose repr is `ffi_name`.
fn conversion_impls(
    rust_name: &Ident,
    ffi_name: &Ident,
    input_body: &TokenStream2,
    return_body: &TokenStream2,
) -> TokenStream2 {
    quote! {
        impl ::rorolala_utils_lazyffi::InputType for #rust_name {
            type From = #ffi_name;

            unsafe fn input_type(input: Self::From) -> Self {
                #input_body
            }
        }

        impl ::rorolala_utils_lazyffi::ReturnType for #rust_name {
            type Target = #ffi_name;

            fn return_self(self) -> Self::Target {
                #return_body
            }
        }

        impl ::rorolala_utils_lazyffi::InputPtr for #rust_name {
            type From = #ffi_name;

            unsafe fn input_ptr(input: *mut Self::From) -> Self {
                // SAFETY: the caller guarantees a valid, aligned, readable pointer.
                unsafe { <Self as ::rorolala_utils_lazyffi::InputType>::input_type(*input) }
            }
        }

        impl ::rorolala_utils_lazyffi::ReturnPtr for #rust_name {
            type Target = #ffi_name;

            fn return_ptr(self) -> *const Self::Target {
                ::std::boxed::Box::into_raw(::std::boxed::Box::new(
                    <Self as ::rorolala_utils_lazyffi::ReturnType>::return_self(self),
                ))
            }
        }
    }
}

/// The generated pieces of one `fn` wrapper.
#[derive(Debug, Default)]
struct FnParts {
    /// `extern "C"` parameter declarations.
    params: Vec<TokenStream2>,
    /// Conversions run before the call.
    prelude: Vec<TokenStream2>,
    /// Arguments passed to the original function.
    call_args: Vec<TokenStream2>,
    /// Write-backs of `&mut` parameters, run after the call.
    write_back: Vec<TokenStream2>,
}

impl FnParts {
    /// Appends `other`'s pieces after `self`'s.
    ///
    /// Used to put a method's receiver in front of its remaining parameters.
    fn append(&mut self, other: Self) {
        self.params.extend(other.params);
        self.prelude.extend(other.prelude);
        self.call_args.extend(other.call_args);
        self.write_back.extend(other.write_back);
    }
}

/// Builds the parameter pieces of an annotated `fn`.
fn expand_fn_params(inputs: &Punctuated<FnArg, Token![,]>) -> syn::Result<FnParts> {
    let mut parts = FnParts::default();

    for input in inputs {
        let FnArg::Typed(pat_type) = input else {
            return Err(syn::Error::new_spanned(
                input,
                "`#[lazyffi]` does not support methods; use an `impl` block",
            ));
        };

        let Pat::Ident(pat_ident) = &*pat_type.pat else {
            return Err(syn::Error::new_spanned(
                &pat_type.pat,
                "`#[lazyffi]` requires plain identifier parameters",
            ));
        };

        let name = &pat_ident.ident;

        if let Type::Reference(reference) = &*pat_type.ty {
            if reference.mutability.is_none() {
                return Err(syn::Error::new_spanned(
                    &pat_type.ty,
                    "`#[lazyffi]` rejects `&T` parameters: a shared reference would let the C side \
                     mutate memory Rust treats as immutable",
                ));
            }

            let inner = &reference.elem;
            let local = format_ident!("__lazyffi_{name}");

            parts.params.push(quote! {
                #name: *mut <#inner as ::rorolala_utils_lazyffi::InputPtr>::From
            });
            parts.prelude.push(quote! {
                let mut #local = unsafe {
                    <#inner as ::rorolala_utils_lazyffi::InputPtr>::input_ptr(#name)
                };
            });
            parts.call_args.push(quote! { &mut #local });
            parts.write_back.push(quote! {
                unsafe {
                    *#name = <#inner as ::rorolala_utils_lazyffi::ReturnType>::return_self(#local);
                }
            });

            continue;
        }

        let ty = &pat_type.ty;

        parts.params.push(quote! {
            #name: <#ty as ::rorolala_utils_lazyffi::InputType>::From
        });
        parts.prelude.push(quote! {
            let #name = unsafe { <#ty as ::rorolala_utils_lazyffi::InputType>::input_type(#name) };
        });
        parts.call_args.push(quote! { #name });
    }

    Ok(parts)
}

/// Builds the `extern "C"` wrapper around a signature.
///
/// `call` is the path the wrapper calls (`foo` for a free function,
/// `Self::foo` for a method) and `parts` are its parameters, receiver included.
///
/// The wrapper is emitted verbatim, without a body of its own: for a method it
/// goes **inside** the original `impl`, so `Self` in the signature keeps meaning
/// what it meant before.
fn wrapper_tokens(
    sig: &Signature,
    attrs: &[Attribute],
    ffi_name: &Ident,
    label: &str,
    call: &TokenStream2,
    parts: FnParts,
) -> syn::Result<TokenStream2> {
    if sig.asyncness.is_some() {
        return Err(syn::Error::new_spanned(
            sig,
            "`#[lazyffi]` does not support `async fn`",
        ));
    }

    if !sig.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &sig.generics,
            "`#[lazyffi]` does not support generic functions",
        ));
    }

    if !matches!(sig.safety, syn::Safety::Default) {
        return Err(syn::Error::new_spanned(
            sig,
            "`#[lazyffi]` does not support `unsafe fn` or `safe fn`",
        ));
    }

    let FnParts {
        params: ffi_params,
        prelude,
        call_args,
        write_back,
    } = parts;

    let (ffi_return, call, return_expr) = if let ReturnType::Type(_, ty) = &sig.output {
        if let Type::Reference(_) = &**ty {
            return Err(syn::Error::new_spanned(
                &**ty,
                "`#[lazyffi]` cannot return a reference: the wrapper cannot guarantee the \
                 referent outlives the call",
            ));
        }

        (
            quote! { -> <#ty as ::rorolala_utils_lazyffi::ReturnType>::Target },
            quote! { let __lazyffi_result = #call(#(#call_args),*); },
            quote! {
                <#ty as ::rorolala_utils_lazyffi::ReturnType>::return_self(__lazyffi_result)
            },
        )
    } else {
        (
            TokenStream2::new(),
            quote! { #call(#(#call_args),*); },
            TokenStream2::new(),
        )
    };

    let mut ffi_docs = mirrored_attrs(label, attrs);
    ffi_docs.push(quote! { #[doc = #SAFETY_DOC] });

    Ok(quote! {
        #(#ffi_docs)*
        #[unsafe(no_mangle)]
        #[allow(nonstandard_style)]
        pub unsafe extern "C" fn #ffi_name(#(#ffi_params),*) #ffi_return {
            #(#prelude)*
            #call
            #(#write_back)*
            #return_expr
        }
    })
}

/// Expands `#[lazyffi]` on a `fn` item.
fn expand_fn(args: &LazyFfiArgs, item: &ItemFn) -> syn::Result<TokenStream2> {
    let rust_name = &item.sig.ident;
    let ffi_name = args
        .export
        .clone()
        .unwrap_or_else(|| default_value_name(rust_name));

    let parts = expand_fn_params(&item.sig.inputs)?;
    let wrapper = wrapper_tokens(
        &item.sig,
        &item.attrs,
        &ffi_name,
        &format!("`{rust_name}`"),
        &quote! { #rust_name },
        parts,
    )?;

    Ok(quote! {
        #item

        #wrapper
    })
}

/// Expands `#[lazyffi]` on an inherent `impl` block.
///
/// Each method becomes an associated `#[unsafe(no_mangle)]` function of the same
/// impl, so `Self` in a method signature keeps its meaning. Methods are exported
/// whether or not they carry their own `#[lazyffi]`; a method-level
/// `#[lazyffi(export = ...)]` only overrides the generated name.
fn expand_impl(args: &LazyFfiArgs, item: &ItemImpl) -> syn::Result<TokenStream2> {
    if let Some((path, _)) = &item.trait_ {
        return Err(syn::Error::new_spanned(
            path,
            "`#[lazyffi]` only supports inherent `impl` blocks, not trait implementations",
        ));
    }

    if !item.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &item.generics,
            "`#[lazyffi]` does not support generic types",
        ));
    }

    if let Some(export) = &args.export {
        return Err(syn::Error::new_spanned(
            export,
            "`export` is not accepted on `impl`, since it would name every method the same; \
             put `#[lazyffi(export = ...)]` on the method instead",
        ));
    }

    let self_name = simple_type_name(&item.self_ty)?;
    let mut cleaned = item.clone();
    let mut wrappers = Vec::new();

    for impl_item in &mut cleaned.items {
        let ImplItem::Fn(method) = impl_item else {
            continue;
        };

        let (export, attrs) = strip_method_lazyffi(&method.attrs)?;
        method.attrs = attrs;

        let rust_name = &method.sig.ident;
        let ffi_name = export.unwrap_or_else(|| {
            Ident::new(
                &method_name(&self_name.to_string(), &rust_name.to_string()),
                rust_name.span(),
            )
        });

        // The receiver is the method's first parameter; everything after it is an
        // ordinary parameter, converted exactly like a free function's.
        let has_receiver = matches!(method.sig.inputs.first(), Some(FnArg::Receiver(_)));
        let mut parts = if let Some(FnArg::Receiver(receiver)) = method.sig.inputs.first() {
            expand_receiver(receiver)?
        } else {
            FnParts::default()
        };
        let rest = method
            .sig
            .inputs
            .iter()
            .skip(usize::from(has_receiver))
            .cloned()
            .collect::<Punctuated<FnArg, Token![,]>>();
        parts.append(expand_fn_params(&rest)?);

        wrappers.push(wrapper_tokens(
            &method.sig,
            &method.attrs,
            &ffi_name,
            &format!("`{self_name}::{rust_name}`"),
            &quote! { Self::#rust_name },
            parts,
        )?);
    }

    cleaned
        .items
        .extend(wrappers.into_iter().map(ImplItem::Verbatim));

    Ok(quote! { #cleaned })
}

/// The name of an `impl` target, rejecting anything but a plain path.
fn simple_type_name(ty: &Type) -> syn::Result<Ident> {
    if let Type::Path(type_path) = ty
        && type_path.qself.is_none()
        && let Some(segment) = type_path.path.segments.last()
        && segment.arguments.is_none()
    {
        return Ok(segment.ident.clone());
    }

    Err(syn::Error::new_spanned(
        ty,
        "`#[lazyffi]` needs a plain path as the `impl` target",
    ))
}

/// Removes a method's own `#[lazyffi]` attribute, returning its `export` override.
///
/// The attribute is consumed here rather than expanded on its own, because the
/// `impl` as a whole is what generates the wrappers.
fn strip_method_lazyffi(attrs: &[Attribute]) -> syn::Result<(Option<Ident>, Vec<Attribute>)> {
    let mut export = None;
    let mut kept = Vec::new();

    for attr in attrs {
        let is_lazyffi = attr
            .path()
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "lazyffi");

        if !is_lazyffi {
            kept.push(attr.clone());
            continue;
        }

        if !matches!(attr.meta, Meta::Path(_)) {
            export = attr.parse_args::<LazyFfiArgs>()?.export;
        }
    }

    Ok((export, kept))
}

/// Builds the parameter pieces of a method receiver.
fn expand_receiver(receiver: &Receiver) -> syn::Result<FnParts> {
    let mut parts = FnParts::default();
    let param = quote! { __lazyffi_self };
    let local = quote! { __lazyffi_receiver };

    match &receiver.kind {
        ReceiverKind::Reference(_, _, mutability) => {
            if mutability.is_none() {
                return Err(syn::Error::new_spanned(
                    receiver,
                    "`#[lazyffi]` rejects `&self`: a shared reference would let the C side mutate \
                     memory Rust treats as immutable",
                ));
            }

            parts.params.push(quote! {
                #param: *mut <Self as ::rorolala_utils_lazyffi::InputPtr>::From
            });
            parts.prelude.push(quote! {
                let mut #local = unsafe {
                    <Self as ::rorolala_utils_lazyffi::InputPtr>::input_ptr(#param)
                };
            });
            parts.call_args.push(quote! { &mut #local });
            parts.write_back.push(quote! {
                unsafe {
                    *#param = <Self as ::rorolala_utils_lazyffi::ReturnType>::return_self(#local);
                }
            });
        }
        ReceiverKind::Value => {
            let binding = if receiver.mutability.is_some() {
                quote! { let mut #local }
            } else {
                quote! { let #local }
            };

            parts.params.push(quote! {
                #param: <Self as ::rorolala_utils_lazyffi::InputType>::From
            });
            parts.prelude.push(quote! {
                #binding = unsafe {
                    <Self as ::rorolala_utils_lazyffi::InputType>::input_type(#param)
                };
            });
            parts.call_args.push(quote! { #local });
        }
        // `ReceiverKind` is non-exhaustive; anything but the two supported
        // shorthands (`self: Box<Self>` today) is rejected.
        _ => {
            return Err(syn::Error::new_spanned(
                receiver,
                "`#[lazyffi]` only supports the `self` and `&mut self` receivers",
            ));
        }
    }

    Ok(parts)
}

/// Whether `ty` is a reference to `str`.
fn is_str_ref(ty: &Type) -> bool {
    if let Type::Reference(reference) = ty
        && let Type::Path(inner) = &*reference.elem
    {
        return inner.path.is_ident("str");
    }

    false
}

/// Whether `ty` is a scalar type usable as a `static`.
fn is_scalar(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty
        && let Some(ident) = type_path.path.get_ident()
    {
        return SCALAR_TYPES.contains(&ident.to_string().as_str());
    }

    false
}

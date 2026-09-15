#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, format_ident, quote};
use rorolala_utils_lazyffi_core::{type_name, value_name};
use syn::{
    Attribute, Expr, ExprLit, ExprPath, Fields, FnArg, Ident, Item, ItemConst, ItemFn, ItemStruct,
    Lit, Meta, Pat, ReturnType, Token, Type,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
};

/// Scalar types that can be exported as a `static`.
const SCALAR_TYPES: &[&str] = &[
    "bool", "char", "f32", "f64", "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32",
    "u64", "u128", "usize",
];

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
/// | `fn` | Supported: by-value and `&mut` parameters, by-value return. |
/// | `enum` | Not implemented yet. |
/// | `impl` | Not implemented yet. |
///
/// Every Rust type has a single repr-C sibling named by `export`; all four
/// conversion traits point at it. The original item is always kept and the FFI
/// item is generated next to it:
///
/// - a `const` scalar becomes a `#[unsafe(no_mangle)] static`, a `const` `&str`
///   becomes an `extern "C"` function allocating a C string (release it with
///   `rorolala_utils_lazyffi::ffi_free_string`),
/// - a `struct` gains a `#[repr(C)]` sibling plus the four conversions,
/// - a `fn` gains one `unsafe extern "C"` wrapper.
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
        Item::Struct(item) => expand_struct(args, item),
        Item::Fn(item) => expand_fn(args, item),
        Item::Enum(item) => Err(not_implemented("enum", item)),
        Item::Impl(item) => Err(not_implemented("impl", item)),
        other => Err(syn::Error::new_spanned(
            other,
            "`#[lazyffi]` can be applied to `const`, `enum`, `struct`, `fn` or `impl` items",
        )),
    }
}

/// Builds a "not implemented yet" error pointing at the offending item.
fn not_implemented(kind: &str, tokens: &impl ToTokens) -> syn::Error {
    syn::Error::new_spanned(
        tokens,
        format!("`#[lazyffi]` on `{kind}` is not implemented yet"),
    )
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

    Ok(quote! {
        #item

        #(#struct_docs)*
        #[repr(C)]
        #[derive(Clone, Copy)]
        #[allow(nonstandard_style)]
        pub struct #ffi_name {
            #(#ffi_fields)*
        }

        impl ::rorolala_utils_lazyffi::InputType for #rust_name {
            type From = #ffi_name;

            unsafe fn input_type(input: Self::From) -> Self {
                Self { #(#from_ffi)* }
            }
        }

        impl ::rorolala_utils_lazyffi::ReturnType for #rust_name {
            type Target = #ffi_name;

            fn return_self(self) -> Self::Target {
                #ffi_name { #(#to_ffi)* }
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
    })
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

/// Expands `#[lazyffi]` on a `fn` item.
fn expand_fn(args: &LazyFfiArgs, item: &ItemFn) -> syn::Result<TokenStream2> {
    if item.sig.asyncness.is_some() {
        return Err(syn::Error::new_spanned(
            &item.sig,
            "`#[lazyffi]` does not support `async fn`",
        ));
    }

    if !item.sig.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &item.sig.generics,
            "`#[lazyffi]` does not support generic functions",
        ));
    }

    if !matches!(item.sig.safety, syn::Safety::Default) {
        return Err(syn::Error::new_spanned(
            &item.sig,
            "`#[lazyffi]` does not support `unsafe fn` or `safe fn`",
        ));
    }

    let rust_name = &item.sig.ident;
    let ffi_name = args
        .export
        .clone()
        .unwrap_or_else(|| default_value_name(rust_name));

    let FnParts {
        params: ffi_params,
        prelude,
        call_args,
        write_back,
    } = expand_fn_params(&item.sig.inputs)?;

    let (ffi_return, call, return_expr) = if let ReturnType::Type(_, ty) = &item.sig.output {
        if let Type::Reference(_) = &**ty {
            return Err(syn::Error::new_spanned(
                &**ty,
                "`#[lazyffi]` cannot return a reference: the wrapper cannot guarantee the \
                 referent outlives the call",
            ));
        }

        (
            quote! { -> <#ty as ::rorolala_utils_lazyffi::ReturnType>::Target },
            quote! { let __lazyffi_result = #rust_name(#(#call_args),*); },
            quote! {
                <#ty as ::rorolala_utils_lazyffi::ReturnType>::return_self(__lazyffi_result)
            },
        )
    } else {
        (
            TokenStream2::new(),
            quote! { #rust_name(#(#call_args),*); },
            TokenStream2::new(),
        )
    };

    let mut ffi_docs = mirrored_attrs(&format!("`{rust_name}`"), &item.attrs);
    ffi_docs.push(quote! { #[doc = #SAFETY_DOC] });

    Ok(quote! {
        #item

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

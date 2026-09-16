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
    PAYLOAD_UNIT_FIELD, VariantFields, free_name, method_name, payload_type_name, tag_type_name,
    type_name, value_name, variant_needs_companion, variant_type_name,
};
use syn::{
    Attribute, Expr, ExprLit, ExprPath, Fields, FnArg, GenericArgument, Ident, ImplItem, Item,
    ItemConst, ItemEnum, ItemFn, ItemImpl, ItemStruct, Lit, Meta, Pat, PathArguments, Receiver,
    ReceiverKind, ReturnType, Signature, Token, Type, Variant,
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
/// | `struct` | Supported: exported as an **opaque handle**. C is told the type exists and nothing about what is in it. No generics. |
/// | `enum` | Supported: unit-only becomes a `#[repr(C)]` enum, variants with data become a tag plus a payload union. |
/// | `fn` | Supported: by-value, `&mut`, `&str` and `&Path` parameters, by-value return. |
/// | `impl` | Supported: inherent impls, flattened into free functions whose first parameter is the receiver. |
///
/// A `struct`'s repr is opaque, so a value of one crosses the boundary only as a
/// pointer: `&mut` borrows it, a by-value parameter **takes ownership** of it, and a
/// return hands out an owning pointer that C releases with `free_<type>`. This is
/// also the only reason a resource whose fields have no repr-C sibling (a `PathBuf`,
/// say) can be exported at all — its fields are never converted.
///
/// An `enum`'s repr is transparent, because its tag and payload *are* its interface:
/// C has to be able to read them. The original item is always kept and the FFI item is
/// generated next to it:
///
/// - a `const` scalar becomes a `#[unsafe(no_mangle)] static`, a `const` `&str`
///   becomes an `extern "C"` function allocating a C string (release it with
///   `rorolala_utils_lazyffi::free_string`),
/// - a `struct` gains a one-field repr wrapping the value, the four conversions, and
///   a `free_<type>` release,
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
/// `fn`/`const`, `ffi_<snake_case(type)>_<snake_case(method)>` for methods,
/// `free_<snake_case(type)>` for a type's release, and `FFI<PascalCase>` for
/// `struct`/`enum` (with `Tag`, `Payload` and `<repr><Variant>` derivatives for the
/// parts of a data-carrying enum). The repr numbers enum variants from zero in
/// declaration order; the Rust discriminants are not mirrored, because the
/// conversions match on the variant rather than reinterpreting the value.
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
/// ```
/// use rorolala_utils_lazyffi::lazyffi;
///
/// /// A rectangle.
/// ///
/// /// # FFI
/// /// Moved in and out by pointer; the handle owns its storage.
/// #[lazyffi]
/// pub struct Rect {}
///
/// fn main() {}
/// ```
///
/// # References
///
/// `&mut T` parameters are supported: the value is read out, the function runs with
/// `&mut`, and the result is written back through the same pointer.
///
/// `&T` parameters and a `&self` receiver are supported too, as read-only borrows: the
/// value is never taken, so C keeps it and the borrow ends with the call. A borrow needs
/// a repr that *is* the value in place, which an exported `struct` has and a scalar has;
/// a `&enum` therefore does not compile, because a borrow of one would have to point at
/// a copy the wrapper had to build.
///
/// ```
/// use rorolala_utils_lazyffi::lazyffi;
///
/// #[lazyffi]
/// pub struct Counter {
///     value: i64,
/// }
///
/// #[lazyffi]
/// impl Counter {
///     /// Borrows the counter; nothing is taken and nothing is written back.
///     pub const fn value(&self) -> i64 {
///         self.value
///     }
///
///     /// Advances the counter where it lies.
///     pub const fn advance(&mut self) {
///         self.value += 1;
///     }
/// }
///
/// /// Borrows a counter the caller still owns.
/// #[lazyffi]
/// pub const fn total(counter: &Counter) -> i64 {
///     counter.value
/// }
///
/// fn main() {}
/// ```
///
/// **`&str` and `&Path`** name something a C string carries rather than a value C
/// holds, so they are built from the C string first and the call borrows the result —
/// which is why releasing that buffer stays the caller's business. A `String` or
/// `PathBuf` parameter is the same thing with the copy made explicit.
///
/// Reference *returns* are rejected: the wrapper cannot guarantee the referent outlives
/// the call.
///
/// # Fallible returns
///
/// A `Result<T, E>` return crosses as the one fixed `RorolalaResult` rather than as a
/// repr generated for it. One C layout cannot hold two arbitrary payload types, and a
/// conversion per `(T, E)` pair would collide the moment two exports returned the same
/// one — so the payload is an owned `void *`, the header names what each side of the
/// tag holds, and the caller casts and releases it.
///
/// `T` and `E` must each own a pointer of their own: an exported `struct` (whose repr
/// already is one), a `String` or `PathBuf`, an exported `enum` (boxed, and released
/// with the `free_<type>` generated beside it), or `()`, which is the `Ok` of the
/// common `Result<(), E>` and crosses as a null payload. Anything else — a scalar, in
/// particular — does not compile.
///
/// A `String` or `PathBuf` **error** is rejected outright: an error is a case the
/// caller switches on, and a sentence gives it nothing to switch on.
///
/// ```
/// use rorolala_utils_lazyffi::lazyffi;
///
/// /// Why a counter refused.
/// #[lazyffi]
/// pub enum CounterError {
///     /// There is no counter to start from.
///     Exhausted,
/// }
///
/// /// A counter.
/// #[lazyffi]
/// pub struct Counter {
///     value: i64,
/// }
///
/// /// Reads a counter, or says why it could not.
/// #[lazyffi]
/// pub fn read(counter: &Counter) -> Result<String, CounterError> {
///     Ok(counter.value.to_string())
/// }
///
/// /// Starts a counter, or says why it would not.
/// #[lazyffi]
/// pub const fn open() -> Result<Counter, CounterError> {
///     Err(CounterError::Exhausted)
/// }
///
/// fn main() {}
/// ```
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

/// The kind of default name an item's repr takes, when `export` does not say.
#[derive(Clone, Copy)]
enum DefaultName {
    /// `FFI<PascalCase>`, for a `struct` or an `enum`.
    Type,
    /// `ffi_<snake_case>`, for a `const` or a `fn`.
    Value,
}

/// The name of the generated item, refusing one that is the item's own.
///
/// `export` names the repr, which is generated *beside* the item it mirrors — so naming
/// it after that item asks for the same name twice in one module. Saying so here beats
/// the pile of resolution errors the collision would otherwise produce.
fn exported_name(
    args: &LazyFfiArgs,
    rust_name: &Ident,
    default: DefaultName,
) -> syn::Result<Ident> {
    let name = args.export.clone().unwrap_or_else(|| match default {
        DefaultName::Type => default_type_name(rust_name),
        DefaultName::Value => default_value_name(rust_name),
    });

    reject_self_export(&name, rust_name)?;

    Ok(name)
}

/// The name C knows an item by: its `export` where it has one, and its Rust name
/// where it does not.
///
/// This is what an `export` is for here — the generated release mirrors the name the
/// caller writes in C, so two types one Rust name apart still have one release each.
fn mirrored_name(args: &LazyFfiArgs, rust_name: &Ident) -> String {
    args.export
        .as_ref()
        .map_or_else(|| rust_name.to_string(), ToString::to_string)
}

/// Refuses an `export` that names the item it mirrors.
fn reject_self_export(name: &Ident, rust_name: &Ident) -> syn::Result<()> {
    if name == rust_name {
        return Err(syn::Error::new_spanned(
            rust_name,
            format!(
                "`export = {name}` names the item it mirrors; the generated repr needs a \
                 name of its own"
            ),
        ));
    }

    Ok(())
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
    let ffi_name = exported_name(args, rust_name, DefaultName::Value)?;
    let ty = &konst.ty;
    let mirrored = mirrored_attrs(&format!("`{rust_name}`"), &konst.attrs);

    if is_str_ref(ty) {
        return Ok(quote! {
            #konst

            #(#mirrored)*
            #[doc(hidden)]
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
            #[doc(hidden)]
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
    let ffi_name = exported_name(args, rust_name, DefaultName::Type)?;
    let release = Ident::new(
        &free_name(&mirrored_name(args, rust_name)),
        rust_name.span(),
    );

    let struct_docs = mirrored_attrs(&format!("`{rust_name}`"), &item.attrs);
    let release_docs = generated_attrs(
        &format!("Releases a `{rust_name}` handed out by an export."),
        &item.attrs,
    );

    Ok(quote! {
        #item

        #(#struct_docs)*
        /// Opaque to C, which is told the type exists and nothing else about it.
        #[doc(hidden)]
        #[repr(transparent)]
        #[allow(nonstandard_style)]
        pub struct #ffi_name(#rust_name);

        #[doc(hidden)]
        impl ::rorolala_utils_lazyffi::InputType for #rust_name {
            type From = *mut #ffi_name;

            /// Takes the value C points at.
            ///
            /// The handle *is* the value, so this moves it out and leaves C's
            /// storage stale: the caller gives it up in exchange.
            unsafe fn input_type(input: Self::From) -> Self {
                // SAFETY: the caller guarantees a valid, aligned, readable pointer to
                // a live value, and hands ownership of that storage over with it.
                unsafe { ::core::ptr::read(::core::ptr::addr_of!((*input).0)) }
            }
        }

        #[doc(hidden)]
        impl ::rorolala_utils_lazyffi::ReturnType for #rust_name {
            type Target = *mut #ffi_name;

            /// Moves the value into freshly allocated storage.
            ///
            /// C owns the result and releases it with [`#release`].
            fn return_self(self) -> Self::Target {
                ::std::boxed::Box::into_raw(::std::boxed::Box::new(#ffi_name(self)))
            }
        }

        #[doc(hidden)]
        impl ::rorolala_utils_lazyffi::InputPtr for #rust_name {
            type From = #ffi_name;

            /// Moves the value out of C's storage, to be handed back by
            /// [`write_ptr`](::rorolala_utils_lazyffi::InputPtr::write_ptr).
            unsafe fn input_ptr(input: *mut Self::From) -> Self {
                // SAFETY: the caller guarantees a valid, aligned, readable pointer,
                // and writes a value back before its storage is used again.
                unsafe { ::core::ptr::read(::core::ptr::addr_of!((*input).0)) }
            }

            unsafe fn write_ptr(self, target: *mut Self::From) {
                // SAFETY: the caller guarantees a valid, aligned, writable pointer,
                // and passes the one the value was read from.
                unsafe { ::core::ptr::write(::core::ptr::addr_of_mut!((*target).0), self) };
            }
        }

        #[doc(hidden)]
        impl ::rorolala_utils_lazyffi::InputRef for #rust_name {
            type From = #ffi_name;

            /// Borrows the value C points at, without taking it.
            ///
            /// The handle is `repr(transparent)` over the value, so a borrow of the
            /// one is a borrow of the other. C keeps both.
            unsafe fn input_ref<'a>(input: *const Self::From) -> &'a Self {
                // SAFETY: the caller guarantees a valid, aligned, readable pointer
                // that stays readable for the duration of the borrow.
                unsafe { &(*input).0 }
            }
        }

        #[doc(hidden)]
        impl ::rorolala_utils_lazyffi::ReturnPtr for #rust_name {
            type Target = #ffi_name;

            fn return_ptr(self) -> *const Self::Target {
                ::std::boxed::Box::into_raw(::std::boxed::Box::new(#ffi_name(self)))
            }
        }

        #[doc(hidden)]
        impl ::rorolala_utils_lazyffi::ResultPayload for #rust_name {
            fn into_payload(self) -> *mut ::core::ffi::c_void {
                // An opaque value already crosses as an owning pointer, so it becomes a
                // payload with nothing further — released with this type's own
                // `free_<type>`.
                <Self as ::rorolala_utils_lazyffi::ReturnType>::return_self(self).cast()
            }
        }

        #(#release_docs)*
        #[doc = #SAFETY_DOC]
        #[doc(hidden)]
        #[unsafe(no_mangle)]
        #[allow(nonstandard_style)]
        pub unsafe extern "C" fn #release(value: *mut #ffi_name) {
            if value.is_null() {
                return;
            }

            // SAFETY: the caller guarantees `value` is a pointer this export handed
            // out and has not been released yet.
            drop(unsafe { ::std::boxed::Box::from_raw(value) });
        }
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
    let ffi_name = exported_name(args, rust_name, DefaultName::Type)?;

    let carries_data = item
        .variants
        .iter()
        .any(|variant| !matches!(variant.fields, Fields::Unit));

    let expansion = if carries_data {
        expand_data_enum(item, rust_name, &ffi_name)
    } else {
        expand_unit_enum(item, rust_name, &ffi_name)
    };

    let release = enum_release(args, rust_name, &ffi_name);

    Ok(quote! {
        #expansion

        #release
    })
}

/// The release C frees a boxed `enum` repr with.
///
/// A transparent type crosses by value, so no ordinary signature needs to release one.
/// A `RorolalaResult` payload is a pointer, though, and the one it carries may name
/// this enum — so there has to be a release for it, named the way every other release
/// is. It frees the box and nothing else: the repr's own fields are the caller's to
/// release one by one, exactly as they are when the enum is handed over by value.
fn enum_release(args: &LazyFfiArgs, rust_name: &Ident, ffi_name: &Ident) -> TokenStream2 {
    // Named the way C knows the type: its export where it has one, its Rust name where
    // it does not.
    let release = Ident::new(
        &free_name(&mirrored_name(args, rust_name)),
        rust_name.span(),
    );
    let docs = generated_attrs(
        &format!("Releases a boxed `{rust_name}` handed out as a result payload."),
        &[],
    );

    quote! {
        #(#docs)*
        #[doc = #SAFETY_DOC]
        #[doc(hidden)]
        #[unsafe(no_mangle)]
        #[allow(nonstandard_style)]
        pub unsafe extern "C" fn #release(value: *mut #ffi_name) {
            if value.is_null() {
                return;
            }

            // SAFETY: the caller guarantees `value` is a pointer this export handed
            // out and has not been released yet.
            drop(unsafe { ::std::boxed::Box::from_raw(value) });
        }
    }
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
        #[doc(hidden)]
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
        #[doc(hidden)]
        #[repr(C)]
        #[derive(Clone, Copy)]
        #[allow(nonstandard_style, clippy::enum_variant_names)]
        pub enum #tag_name {
            #(#tag_variants),*
        }

        #(#payload_docs)*
        #[doc(hidden)]
        #[repr(C)]
        #[derive(Clone, Copy)]
        #[allow(nonstandard_style, clippy::pub_underscore_fields)]
        pub union #payload_name {
            #(#union_fields)*
        }

        #(#docs)*
        #[doc(hidden)]
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
            #[doc(hidden)]
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
        #[doc(hidden)]
        impl ::rorolala_utils_lazyffi::InputType for #rust_name {
            type From = #ffi_name;

            unsafe fn input_type(input: Self::From) -> Self {
                #input_body
            }
        }

        #[doc(hidden)]
        impl ::rorolala_utils_lazyffi::ReturnType for #rust_name {
            type Target = #ffi_name;

            fn return_self(self) -> Self::Target {
                #return_body
            }
        }

        #[doc(hidden)]
        impl ::rorolala_utils_lazyffi::InputPtr for #rust_name {
            type From = #ffi_name;

            unsafe fn input_ptr(input: *mut Self::From) -> Self {
                // SAFETY: the caller guarantees a valid, aligned, readable pointer.
                unsafe { <Self as ::rorolala_utils_lazyffi::InputType>::input_type(*input) }
            }

            unsafe fn write_ptr(self, target: *mut Self::From) {
                // SAFETY: the caller guarantees a valid, aligned, writable pointer.
                unsafe {
                    *target = <Self as ::rorolala_utils_lazyffi::ReturnType>::return_self(self);
                }
            }
        }

        #[doc(hidden)]
        impl ::rorolala_utils_lazyffi::ReturnPtr for #rust_name {
            type Target = #ffi_name;

            fn return_ptr(self) -> *const Self::Target {
                ::std::boxed::Box::into_raw(::std::boxed::Box::new(
                    <Self as ::rorolala_utils_lazyffi::ReturnType>::return_self(self),
                ))
            }
        }

        #[doc(hidden)]
        impl ::rorolala_utils_lazyffi::ResultPayload for #rust_name {
            fn into_payload(self) -> *mut ::core::ffi::c_void {
                // A transparent type is a value and a result payload is a pointer, so
                // the repr is boxed — released with the `free_<type>` beside this.
                ::std::boxed::Box::into_raw(::std::boxed::Box::new(
                    <Self as ::rorolala_utils_lazyffi::ReturnType>::return_self(self),
                ))
                .cast()
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

/// The owned type a borrowed parameter is read into, and how to borrow it back.
///
/// A shared reference is otherwise rejected. `&str` and `&Path` are the exceptions
/// because each names something a C string carries, and Rust only ever reads them:
/// the C string is copied into an owned value first, so releasing it is none of the
/// callee's business.
///
/// Matched on the last path segment, the same way the header generator resolves
/// types, so a qualified `std::path::Path` works too.
fn borrowed_type(elem: &Type) -> Option<(TokenStream2, Ident)> {
    let Type::Path(type_path) = elem else {
        return None;
    };
    if type_path.qself.is_some() {
        return None;
    }

    let ident = &type_path.path.segments.last()?.ident;

    Some(match ident.to_string().as_str() {
        "str" => (
            quote! { ::std::string::String },
            Ident::new("as_str", ident.span()),
        ),
        "Path" => (
            quote! { ::std::path::PathBuf },
            Ident::new("as_path", ident.span()),
        ),
        _ => return None,
    })
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
                // The two shared references that name something a C string can
                // carry are read as one: an owned value is built from the C string
                // and the call borrows it back. What C holds there is the string,
                // not the value, so there is nothing to borrow in place.
                if let Some((owned, borrow)) = borrowed_type(&reference.elem) {
                    parts.params.push(quote! {
                        #name: <#owned as ::rorolala_utils_lazyffi::InputType>::From
                    });
                    parts.prelude.push(quote! {
                        let #name = unsafe {
                            <#owned as ::rorolala_utils_lazyffi::InputType>::input_type(#name)
                        };
                    });
                    parts.call_args.push(quote! { #name.#borrow() });

                    continue;
                }

                // Any other shared reference borrows the value in place and never
                // takes it: C keeps its value, and the borrow ends with the call.
                let inner = &reference.elem;

                parts.params.push(quote! {
                    #name: *const <#inner as ::rorolala_utils_lazyffi::InputRef>::From
                });
                parts.prelude.push(quote! {
                    let #name = unsafe {
                        <#inner as ::rorolala_utils_lazyffi::InputRef>::input_ref(#name)
                    };
                });
                parts.call_args.push(quote! { #name });

                continue;
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
                    <#inner as ::rorolala_utils_lazyffi::InputPtr>::write_ptr(#local, #name);
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

        if let Some(error) = result_error_type(ty)
            && is_string_like(error)
        {
            return Err(syn::Error::new_spanned(
                error,
                "`#[lazyffi]` rejects a `String` or `PathBuf` error: C is given a tag to switch \
                 on, and a sentence gives it nothing to switch on. Give each failure case an \
                 exported `enum` instead.",
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
        #[doc(hidden)]
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
    let ffi_name = exported_name(args, rust_name, DefaultName::Value)?;

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
        reject_self_export(&ffi_name, rust_name)?;

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
                // Nothing is taken here either: the receiver is borrowed in place and
                // left alone, so a read-only method costs no move and no write-back.
                parts.params.push(quote! {
                    #param: *const <Self as ::rorolala_utils_lazyffi::InputRef>::From
                });
                parts.prelude.push(quote! {
                    let #local = unsafe {
                        <Self as ::rorolala_utils_lazyffi::InputRef>::input_ref(#param)
                    };
                });
                parts.call_args.push(quote! { #local });

                return Ok(parts);
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
                    <Self as ::rorolala_utils_lazyffi::InputPtr>::write_ptr(#local, #param);
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
        // `ReceiverKind` is non-exhaustive; anything but the three supported
        // shorthands (`self: Box<Self>` today) is rejected.
        _ => {
            return Err(syn::Error::new_spanned(
                receiver,
                "`#[lazyffi]` only supports the `self`, `&self` and `&mut self` receivers",
            ));
        }
    }

    Ok(parts)
}

/// The error type of a `Result<T, E>`, when `ty` is one.
///
/// Matched on the last path segment, the way every other type rule here is: the
/// macro cannot tell `Result` from an alias for it, and pretending otherwise would
/// mean resolving names it has no view of.
fn result_error_type(ty: &Type) -> Option<&Type> {
    let Type::Path(type_path) = ty else {
        return None;
    };

    if type_path.qself.is_some() {
        return None;
    }

    let segment = type_path.path.segments.last()?;
    if segment.ident != "Result" {
        return None;
    }

    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };

    let mut types = arguments.args.iter().filter_map(|argument| match argument {
        GenericArgument::Type(ty) => Some(ty),
        _ => None,
    });

    let _ok = types.next()?;

    types.next()
}

/// Whether `ty` names a string or a path: the two types that cross as C strings.
fn is_string_like(ty: &Type) -> bool {
    let Type::Path(type_path) = ty else {
        return false;
    };

    type_path.path.segments.last().is_some_and(|segment| {
        matches!(
            segment.ident.to_string().as_str(),
            "String" | "PathBuf" | "str" | "Path"
        )
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

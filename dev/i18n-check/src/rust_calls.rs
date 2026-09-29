//! Finds the keys a Rust crate's `t!` calls name.
//!
//! The scan is over tokens rather than over the syntax tree, because a `t!` is almost always
//! written inside another macro — `trd!(t!("…").trim())`, `r_eprintln!("{}", t!("…"))` — and a
//! tree walk sees an outer macro's arguments as one opaque token run, so it never reaches the
//! `t!` inside. Lexing the file and walking every group reaches a `t!` wherever it is written,
//! and reaches none of the `t!`s that appear in doc comments or string literals, since those are
//! not macro tokens.

use std::fs;
use std::path::Path;

use proc_macro2::{TokenStream, TokenTree};
use syn::Lit;

use crate::calls::{Call, Calls};
use crate::files;

/// Reads every `.rs` file under `dir` and collects the keys its `t!` calls name.
///
/// # Errors
///
/// Returns a message naming a file that could not be read or lexed.
pub fn calls(dir: &Path) -> Result<Calls, String> {
    let mut all = Calls::default();

    for file in files::under(dir, &["rs"], &[])? {
        let text =
            fs::read_to_string(&file).map_err(|error| format!("{}: {error}", file.display()))?;
        let tokens: TokenStream = text
            .parse()
            .map_err(|error| format!("{}: {error}", file.display()))?;

        walk(tokens, &file, &mut all);
    }

    Ok(all)
}

/// Walks a token stream, and every group in it, collecting `t!` calls.
fn walk(tokens: TokenStream, file: &Path, calls: &mut Calls) {
    let trees: Vec<TokenTree> = tokens.into_iter().collect();
    let mut at = 0;

    while at < trees.len() {
        match &trees[at] {
            TokenTree::Group(group) => walk(group.stream(), file, calls),
            TokenTree::Ident(ident)
                if ident == "t"
                    && matches!(
                        trees.get(at + 1),
                        Some(TokenTree::Punct(punct)) if punct.as_char() == '!'
                    ) =>
            {
                if let Some(TokenTree::Group(group)) = trees.get(at + 2) {
                    match key_of(group.stream()) {
                        Some(key) => calls.calls.push(Call {
                            file: file.to_path_buf(),
                            line: ident.span().start().line,
                            key,
                        }),
                        None => calls.unchecked += 1,
                    }

                    at += 3;

                    continue;
                }
            }
            _ => {}
        }

        at += 1;
    }
}

/// The key a `t!` call names, when its first argument is a string literal.
fn key_of(tokens: TokenStream) -> Option<String> {
    match tokens.into_iter().next()? {
        TokenTree::Literal(literal) => match Lit::new(literal) {
            Lit::Str(text) => Some(text.value()),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{Calls, walk};

    /// The keys `source` names, and how many calls named one this cannot read.
    fn keys(source: &str) -> (Vec<String>, usize) {
        let mut calls = Calls {
            calls: Vec::new(),
            unchecked: 0,
        };

        walk(source.parse().unwrap(), Path::new("test.rs"), &mut calls);

        (
            calls.calls.into_iter().map(|call| call.key).collect(),
            calls.unchecked,
        )
    }

    #[test]
    fn a_key_is_reached_wherever_it_is_written() {
        let (found, unchecked) = keys(
            r#"
            fn one() { let _ = t!("plain"); }
            fn two() { r_eprintln!("{}", trd!(t!("nested").trim())); }
            fn three() { let _ = rust_i18n::t!("qualified"); }
            "#,
        );

        assert_eq!(found, ["plain", "nested", "qualified"]);
        assert_eq!(unchecked, 0);
    }

    #[test]
    fn a_key_that_is_not_a_literal_is_counted_rather_than_guessed() {
        let (found, unchecked) = keys(r"fn one(key: &str) { let _ = t!(key); }");

        assert!(found.is_empty());
        assert_eq!(unchecked, 1);
    }

    #[test]
    fn a_doc_comment_is_not_a_call_site() {
        // The `t!` here is in a doc comment, which is not a macro token; the other two `t!`s are
        // part of longer names, which are not the `t!` macro either.
        let (found, unchecked) = keys(
            r#"
            /// A `t!("documented")` in a comment names nothing.
            fn one() { assert!(true); }
            fn two() { suggest!(); }
            "#,
        );

        assert!(found.is_empty());
        assert_eq!(unchecked, 0);
    }
}

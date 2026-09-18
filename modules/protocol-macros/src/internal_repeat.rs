use proc_macro::{Delimiter, Group, Ident, Literal, TokenStream, TokenTree};

pub(crate) fn internal_repeat(input: TokenStream) -> TokenStream {
    let tokens: Vec<TokenTree> = input.into_iter().collect();
    let (range_start, range_end, body_start) = parse_range(&tokens);

    let mut body: Vec<TokenTree> = tokens[body_start..].to_vec();
    if body.len() == 1
        && let TokenTree::Group(g) = &body[0]
        && g.delimiter() == Delimiter::Brace
    {
        body = g.stream().into_iter().collect();
    }

    let mut result = Vec::new();
    for i in range_start..=range_end {
        result.extend(expand_body(&body, i, range_start, range_end));
    }
    result.into_iter().collect()
}

/// Parse `start .. end =>` or `start ..= end =>` or `count =>` (backward compat).
/// Returns `(start, end_inclusive, body_start_index)`.
fn parse_range(tokens: &[TokenTree]) -> (usize, usize, usize) {
    // Find => separator
    let arrow_pos = tokens.windows(2).position(|w| {
        matches!(&w[0], TokenTree::Punct(p) if p.as_char() == '=')
            && matches!(&w[1], TokenTree::Punct(p) if p.as_char() == '>')
    });

    let (arrow_pos, body_start) = match arrow_pos {
        Some(p) => (p, p + 2),
        None => return (1, 12, 0), // fallback
    };

    let before: Vec<&TokenTree> = tokens[..arrow_pos].iter().collect();

    // Try to find `..` or `..=` pattern
    // `..` is two Punct('.') tokens
    let dotdot = before.windows(2).position(|w| {
        matches!(w[0], TokenTree::Punct(p) if p.as_char() == '.')
            && matches!(w[1], TokenTree::Punct(p) if p.as_char() == '.')
    });

    if let Some(dd) = dotdot {
        // Start value: tokens before `..`
        let start = parse_usize_tokens(&before[..dd]);
        let after_dd = &before[dd + 2..];

        // Check for `..=` (inclusive range)
        let (inclusive, end_tokens) = if after_dd
            .first()
            .is_some_and(|t| matches!(t, TokenTree::Punct(p) if p.as_char() == '='))
        {
            (true, &after_dd[1..])
        } else {
            (false, after_dd)
        };

        let end = parse_usize_tokens(end_tokens);

        if inclusive {
            (start, end, body_start)
        } else {
            // Exclusive end: if end >= start, iterate start..end, so end_inclusive = end - 1
            if end > start {
                (start, end - 1, body_start)
            } else {
                (1, 12, body_start) // fallback
            }
        }
    } else {
        // No `..` found — fallback to simple count
        let count = parse_usize_tokens(&before);
        (1, count, body_start)
    }
}

/// Parse a sequence of tokens as a single usize value.
fn parse_usize_tokens(tokens: &[&TokenTree]) -> usize {
    let s: String = tokens
        .iter()
        .map(|t| match t {
            TokenTree::Literal(l) => l.to_string(),
            TokenTree::Ident(id) => id.to_string(),
            _ => String::new(),
        })
        .collect::<Vec<_>>()
        .join("")
        .replace(' ', "");

    s.parse().unwrap_or(12)
}

/// Walk tokens, replacing:
///   `$`   → current
///   `^$`  → max
///   `$^`  → min
///   `$+`  → current + 1 (clamped)
///   `$-`  → current - 1 (clamped)
///   `ident$` → ident{current}
///   `ident$suffix` → ident{current}suffix (e.g. `With$Arg` → `With1Arg`).
///                     The suffix is only consumed when `$` is directly
///                     followed by an identifier (after `$+`/`$-`/`$^` are ruled out).
/// and expanding `( … )+` / `( … ,)+` / `( … ;)+` groups.
fn expand_body(tokens: &[TokenTree], current: usize, min: usize, max: usize) -> Vec<TokenTree> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        // Check for a parenthesized repetition group: ( ... ) sep? +
        if let Some(exp) = try_expand_paren_group(tokens, i, current, min, max) {
            let (items, consumed) = exp;
            out.extend(items);
            i += consumed;
            continue;
        }

        // `^$` — max value
        if let TokenTree::Punct(p) = &tokens[i]
            && p.as_char() == '^'
            && i + 1 < tokens.len()
            && let TokenTree::Punct(p2) = &tokens[i + 1]
            && p2.as_char() == '$'
        {
            out.push(TokenTree::Literal(Literal::usize_suffixed(max)));
            i += 2;
            continue;
        }

        // `ident$` / `ident$+` / `ident$-` / `ident$^`
        // → {ident}{current} / {ident}{current+1} / {ident}{current-1} / {ident}{min}
        if let TokenTree::Ident(id) = &tokens[i] {
            if i + 1 < tokens.len()
                && let TokenTree::Punct(p) = &tokens[i + 1]
                && p.as_char() == '$'
            {
                // Check for ident$+ / ident$- / ident$^
                if i + 2 < tokens.len() {
                    match &tokens[i + 2] {
                        TokenTree::Punct(p2) if p2.as_char() == '+' => {
                            // ident$+ → {ident}{current+1}
                            let name = format!("{}{}", id, current + 1);
                            out.push(TokenTree::Ident(Ident::new(&name, id.span())));
                            i += 3;
                            continue;
                        }
                        TokenTree::Punct(p2) if p2.as_char() == '-' => {
                            // ident$- → {ident}{current-1}
                            let val = current.saturating_sub(1);
                            let name = format!("{}{}", id, val);
                            out.push(TokenTree::Ident(Ident::new(&name, id.span())));
                            i += 3;
                            continue;
                        }
                        TokenTree::Punct(p2) if p2.as_char() == '^' => {
                            let name = format!("{}{}", id, min);
                            out.push(TokenTree::Ident(Ident::new(&name, id.span())));
                            i += 3;
                            continue;
                        }
                        // ident$suffix → {ident}{current}{suffix}
                        // e.g. `With$Arg` → `With1Arg`
                        TokenTree::Ident(suffix) => {
                            let name = format!("{}{}{}", id, current, suffix);
                            out.push(TokenTree::Ident(Ident::new(&name, id.span())));
                            i += 3;
                            continue;
                        }
                        _ => {}
                    }
                }
                // ident$ alone → {ident}{current}
                let name = format!("{}{}", id, current);
                out.push(TokenTree::Ident(Ident::new(&name, id.span())));
                i += 2;
                continue;
            }
            out.push(tokens[i].clone());
            i += 1;
            continue;
        }

        match &tokens[i] {
            TokenTree::Punct(p) if p.as_char() == '$' => {
                // lookahead for $^, $+, $-
                if i + 1 < tokens.len() {
                    match &tokens[i + 1] {
                        TokenTree::Punct(p2) if p2.as_char() == '^' => {
                            // $^ → min
                            out.push(TokenTree::Literal(Literal::usize_suffixed(min)));
                            i += 2;
                            continue;
                        }
                        TokenTree::Punct(p2) if p2.as_char() == '+' => {
                            // $+ → current + 1
                            out.push(TokenTree::Literal(Literal::usize_suffixed(current + 1)));
                            i += 2;
                            continue;
                        }
                        TokenTree::Punct(p2) if p2.as_char() == '-' => {
                            // $- → current - 1
                            let val = current.saturating_sub(1);
                            out.push(TokenTree::Literal(Literal::usize_suffixed(val)));
                            i += 2;
                            continue;
                        }
                        _ => {}
                    }
                }
                // `$` alone → current
                out.push(TokenTree::Literal(Literal::usize_suffixed(current)));
                i += 1;
                continue;
            }
            TokenTree::Group(g) => {
                let inner = expand_body_vec(&g.stream(), current, min, max);
                out.push(TokenTree::Group(Group::new(
                    g.delimiter(),
                    inner.into_iter().collect(),
                )));
            }
            other => out.push(other.clone()),
        }
        i += 1;
    }
    out
}

fn expand_body_vec(stream: &TokenStream, current: usize, min: usize, max: usize) -> Vec<TokenTree> {
    let v: Vec<TokenTree> = stream.clone().into_iter().collect();
    expand_body(&v, current, min, max)
}

/// Try to expand a repetition group.
///
/// New syntax (repetition marker `+` is the LAST token INSIDE the parens):
/// - `(group,+)`    — repeat `*` times (current), `,` is the separator
/// - `(group,+)`    — repeat `*` times, `,` is the separator
/// - `(group,+)`    — repeat `*` times, `;` is the separator
/// - `(group +)`    — repeat `*` times, no separator
/// - `(group,+)+`   — repeat `*+1` times (with separator)
/// - `(group,+)--`   — repeat `*-1` times (with separator) [not yet used]
/// - `(group,+)^`   — repeat `max - *` times (with separator); the complement,
///   used to pad a fixed-length list with the items `*` does not produce.
///
/// The `*` is the current counter value. An optional `+`, `-` or `^` immediately
/// after the closing paren shifts the repeat count up/down by one, or makes it
/// the complement of `max`.
fn try_expand_paren_group(
    tokens: &[TokenTree],
    i: usize,
    current: usize,
    _min: usize,
    max: usize,
) -> Option<(Vec<TokenTree>, usize)> {
    let group = match tokens.get(i)? {
        TokenTree::Group(g) if g.delimiter() == Delimiter::Parenthesis => g,
        _ => return None,
    };

    let stream: Vec<TokenTree> = group.stream().into_iter().collect();

    // Repetition syntax: the LAST token inside the parens MUST be `+`.
    //   `(content,+)`      — repeat * times, `,` separator
    //   `(content;+)`      — repeat * times, `;` separator
    //   `(content,+)`      — repeat * times, no separator
    //   `(content,+)+`     — repeat *+1 times
    //   `(content,+)-`     — repeat *-1 times
    // An optional `+` / `-` right after `)` shifts the count by ±1.
    //
    // Any `+` tokens inside `content` are treated as regular Rust syntax
    // (trait bounds, etc.) — the repetition marker is ONLY the final `+`.

    let last_is_plus = stream
        .last()
        .is_some_and(|t| matches!(t, TokenTree::Punct(p) if p.as_char() == '+'));
    if !last_is_plus {
        return None;
    }

    // Determine separator and inner content.
    let (inner, sep_str): (Vec<TokenTree>, &str) = {
        if stream.len() >= 2 {
            let sep_idx = stream.len() - 2;
            match &stream[sep_idx] {
                TokenTree::Punct(p) if p.as_char() == ',' => {
                    // (...,content,+) — inner is everything before the last `,`
                    let content: Vec<TokenTree> = stream[..sep_idx].into();
                    (content, ",")
                }
                TokenTree::Punct(p) if p.as_char() == ';' => {
                    let content: Vec<TokenTree> = stream[..sep_idx].into();
                    (content, ";")
                }
                _ => {
                    // (content+) — no separator, the `+` is the only special token
                    let content: Vec<TokenTree> = stream[..stream.len() - 1].into();
                    (content, "")
                }
            }
        } else {
            // (+) — bare repetition marker, empty content
            (vec![], "")
        }
    };

    // Handle modifier after `)`
    let rest = &tokens[i + 1..];
    let (repeat_count, consumed) = match rest.first() {
        Some(TokenTree::Punct(p)) if p.as_char() == '+' => (current + 1, 2),
        Some(TokenTree::Punct(p)) if p.as_char() == '-' => (current.saturating_sub(1), 2),
        // `^` — repeat `max - current` times. Used to pad a fixed-length
        // parameter list with the parameters not consumed by this arity.
        Some(TokenTree::Punct(p)) if p.as_char() == '^' => (max.saturating_sub(current), 2),
        _ => (current, 1),
    };

    let mut out = Vec::new();
    for n in 1..=repeat_count {
        if n > 1 && !sep_str.is_empty() {
            out.push(TokenTree::Punct(proc_macro::Punct::new(
                sep_str.chars().next().unwrap(),
                proc_macro::Spacing::Alone,
            )));
        }
        out.extend(expand_body(&inner, n, 1, repeat_count));
    }

    Some((out, consumed))
}

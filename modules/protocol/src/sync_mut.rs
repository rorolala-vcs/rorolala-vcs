//! `sync_mut`: changing a value on the side that holds it, and carrying it across.
//!
//! There are two entry points, split by where the side a change lands on comes from:
//!
//! * [`OnlyWorkspace::sync_mut`] and [`OnlyVault::sync_mut`] take no data from outside.
//!   The wrapper already names the side — it holds the value — so the closure is handed
//!   that value alone, and both sides come out holding what it wrote as a [`Both`].
//! * [`Both::sync_mut_with`] changes a value both sides already hold. The side is named
//!   by the read-only inputs the caller passes instead, which is what the `_with` is
//!   about.
//!
//! The input variants for two to twelve values are generated from a single template by
//! `internal_repeat!`. The one-input case is written out instead, because it takes its
//! input bare rather than as a one-element tuple.

use std::future::Future;

use rorolala_protocol_macros::internal_repeat;

use crate::sync::{receive_value, send_value};
use crate::{ActionContext, ActionError, Both, Encodable, OnlyVault, OnlyWorkspace};

impl<T> OnlyWorkspace<T>
where
    T: Encodable + Send + Sync + 'static,
{
    /// Changes the value on the Workspace, and carries it to the Vault.
    ///
    /// The Workspace is the side that holds the value, so it is the side that runs the
    /// closure; what the closure wrote is sent to the Vault, and both sides come out
    /// holding it as a [`Both`]. The closure is handed the value as `&mut T`, so it reads
    /// what the Workspace held and writes what both sides are to hold.
    ///
    /// Both sides call this for the same value at the same time. The Vault's own wrapper
    /// is empty, which is how it is built on that side, so the Vault does not run the
    /// closure and cannot disagree about what the change was; it takes what arrives.
    ///
    /// # Errors
    ///
    /// Returns [`ActionError`] if the context has no channel, the Workspace held no value
    /// to change, the value will not cross the channel, or the stream fails.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let input: OnlyWorkspace<String> = ctx.only_workspace(|| "world".to_owned());
    /// let received = input
    ///     .sync_mut(&mut ctx, |raw| *raw = format!("Hello, {raw}"))
    ///     .await?;
    /// assert_eq!(received.into_inner(), "Hello, world");
    /// ```
    pub async fn sync_mut<Run>(
        self,
        ctx: &mut ActionContext,
        run: Run,
    ) -> Result<Both<T>, ActionError>
    where
        Run: FnOnce(&mut T) + Send,
    {
        if ctx.is_workspace() {
            let mut value = self.into_inner().ok_or(ActionError::MissingValue)?;
            run(&mut value);

            let channel = ctx.channel_mut().ok_or(ActionError::NoChannel)?;
            send_value(channel, &value).await?;

            Ok(Both::new(value))
        } else {
            let channel = ctx.channel_mut().ok_or(ActionError::NoChannel)?;
            let value = receive_value(channel).await?;

            Ok(Both::new(value))
        }
    }
}

impl<T> OnlyVault<T>
where
    T: Encodable + Send + Sync + 'static,
{
    /// Changes the value on the Vault, and carries it to the Workspace.
    ///
    /// As [`OnlyWorkspace::sync_mut`], with the two sides swapped: the Vault is the side
    /// that holds the value, so the Vault runs the closure and the Workspace takes what
    /// it produced.
    ///
    /// # Errors
    ///
    /// Returns [`ActionError`] if the context has no channel, the Vault held no value to
    /// change, the value will not cross the channel, or the stream fails.
    pub async fn sync_mut<Run>(
        self,
        ctx: &mut ActionContext,
        run: Run,
    ) -> Result<Both<T>, ActionError>
    where
        Run: FnOnce(&mut T) + Send,
    {
        if ctx.is_vault() {
            let mut value = self.into_inner().ok_or(ActionError::MissingValue)?;
            run(&mut value);

            let channel = ctx.channel_mut().ok_or(ActionError::NoChannel)?;
            send_value(channel, &value).await?;

            Ok(Both::new(value))
        } else {
            let channel = ctx.channel_mut().ok_or(ActionError::NoChannel)?;
            let value = receive_value(channel).await?;

            Ok(Both::new(value))
        }
    }
}

/// What a [`Both::sync_mut_with`](crate::Both::sync_mut_with) input is, and which side
/// runs the closure.
///
/// Implemented for a single [`OnlyWorkspace`] or [`OnlyVault`], and for a tuple of two
/// to twelve of either. A tuple mixing the two, or holding a value that is not wrapped
/// at all, has no implementation, so the compiler rejects it — which is what makes the
/// side a change lands on follow from the inputs rather than from a flag.
///
/// This is the shape [`sync_mut_with`](crate::Both::sync_mut_with) dispatches through; it
/// is an implementation detail, not something to implement or name.
#[doc(hidden)]
pub trait SyncMutWith<T, Run> {
    /// Changes `value` on the side that holds the inputs, then carries it across.
    ///
    /// # Errors
    ///
    /// Returns [`ActionError`] if the context has no channel, the side that holds the
    /// inputs held none to read, the value will not cross the channel, or the stream
    /// fails.
    fn sync_mut_with(
        self,
        ctx: &mut ActionContext,
        value: &mut T,
        run: Run,
    ) -> impl Future<Output = Result<(), ActionError>> + Send;
}

/// The one-input Vault case: the Vault holds the input, so the Vault runs the closure.
impl<T, A, Run> SyncMutWith<T, Run> for OnlyVault<A>
where
    T: Encodable + Send + Sync + 'static,
    A: Send + Sync + 'static,
    Run: FnOnce(&mut T, A) + Send,
{
    async fn sync_mut_with(
        self,
        ctx: &mut ActionContext,
        value: &mut T,
        run: Run,
    ) -> Result<(), ActionError> {
        if ctx.is_vault() {
            let input = self.into_inner().ok_or(ActionError::MissingValue)?;
            run(value, input);

            let channel = ctx.channel_mut().ok_or(ActionError::NoChannel)?;
            send_value(channel, value).await
        } else {
            let channel = ctx.channel_mut().ok_or(ActionError::NoChannel)?;
            *value = receive_value(channel).await?;
            Ok(())
        }
    }
}

/// The one-input Workspace case: the Workspace holds the input, so it runs the closure.
impl<T, A, Run> SyncMutWith<T, Run> for OnlyWorkspace<A>
where
    T: Encodable + Send + Sync + 'static,
    A: Send + Sync + 'static,
    Run: FnOnce(&mut T, A) + Send,
{
    async fn sync_mut_with(
        self,
        ctx: &mut ActionContext,
        value: &mut T,
        run: Run,
    ) -> Result<(), ActionError> {
        if ctx.is_workspace() {
            let input = self.into_inner().ok_or(ActionError::MissingValue)?;
            run(value, input);

            let channel = ctx.channel_mut().ok_or(ActionError::NoChannel)?;
            send_value(channel, value).await
        } else {
            let channel = ctx.channel_mut().ok_or(ActionError::NoChannel)?;
            *value = receive_value(channel).await?;
            Ok(())
        }
    }
}

internal_repeat!(2..=12 => {
    /// Vault-only inputs: the Vault holds them, so the Vault runs the closure.
    impl<T, Run, (A$-, +)> SyncMutWith<T, Run>
        for (
            (
                OnlyVault<A$->,
            +)
            ,
        )
    where
        Run: FnOnce(
            &mut T,
            (
                (
                    A$-,
                +)
                ,
            ),
        ) + Send,
        T: Encodable + Send + Sync + 'static,
        (
            A$-: Send + Sync + 'static,
        +)
    {
        async fn sync_mut_with(
            self,
            ctx: &mut ActionContext,
            value: &mut T,
            run: Run,
        ) -> Result<(), ActionError> {
            let (
                (
                    a$-,
                +)
                ,
            ) = self;

            if ctx.is_vault() {
                let (
                    (
                        i$-,
                    +)
                    ,
                ) = (
                    (
                        a$-.into_inner().ok_or(ActionError::MissingValue)?,
                    +)
                    ,
                );

                run(value, (
                    (
                        i$-,
                    +)
                    ,
                ));

                let channel = ctx.channel_mut().ok_or(ActionError::NoChannel)?;
                send_value(channel, value).await
            } else {
                let channel = ctx.channel_mut().ok_or(ActionError::NoChannel)?;
                *value = receive_value(channel).await?;
                Ok(())
            }
        }
    }

    /// Workspace-only inputs: the Workspace holds them, so it runs the closure.
    impl<T, Run, (A$-, +)> SyncMutWith<T, Run>
        for (
            (
                OnlyWorkspace<A$->,
            +)
            ,
        )
    where
        Run: FnOnce(
            &mut T,
            (
                (
                    A$-,
                +)
                ,
            ),
        ) + Send,
        T: Encodable + Send + Sync + 'static,
        (
            A$-: Send + Sync + 'static,
        +)
    {
        async fn sync_mut_with(
            self,
            ctx: &mut ActionContext,
            value: &mut T,
            run: Run,
        ) -> Result<(), ActionError> {
            let (
                (
                    a$-,
                +)
                ,
            ) = self;

            if ctx.is_workspace() {
                let (
                    (
                        i$-,
                    +)
                    ,
                ) = (
                    (
                        a$-.into_inner().ok_or(ActionError::MissingValue)?,
                    +)
                    ,
                );

                run(value, (
                    (
                        i$-,
                    +)
                    ,
                ));

                let channel = ctx.channel_mut().ok_or(ActionError::NoChannel)?;
                send_value(channel, value).await
            } else {
                let channel = ctx.channel_mut().ok_or(ActionError::NoChannel)?;
                *value = receive_value(channel).await?;
                Ok(())
            }
        }
    }
});

// `internal_repeat!` is a proc macro, so its parser cannot be reached directly from a unit test:
// `proc_macro::TokenStream` values cannot be built outside a live macro invocation. What can be
// pinned is the behaviour of its expansions, which is what these tests do — each one invokes the
// macro with a shape that exercises a specific branch of its parser and asserts on the Rust the
// expansion produces. The silent fallbacks (no `=>`, an empty range, non-numeric bounds) expand to
// the same twelve repetitions, so they are asserted to hold that behaviour rather than to error.
#[cfg(test)]
mod tests {
    use rorolala_protocol_macros::internal_repeat;

    #[test]
    fn an_inclusive_range_repeats_once_per_number() {
        let mut count = 0_u32;
        internal_repeat!(2..=4 => {
            count += 1;
        });
        assert_eq!(count, 3);
    }

    #[test]
    fn an_exclusive_range_stops_before_its_end() {
        let mut count = 0_u32;
        internal_repeat!(2..5 => {
            count += 1;
        });
        assert_eq!(count, 3);
    }

    #[test]
    fn a_bare_number_counts_up_to_itself() {
        let mut sum = 0_usize;
        internal_repeat!(4 => {
            sum += $;
        });
        assert_eq!(sum, 1 + 2 + 3 + 4);
    }

    #[test]
    fn a_missing_arrow_falls_back_to_twelve_repetitions() {
        let mut count = 0_u32;
        internal_repeat!({
            count += 1;
        });
        assert_eq!(count, 12);
    }

    #[test]
    fn an_empty_range_falls_back_to_twelve_repetitions() {
        let mut count = 0_u32;
        internal_repeat!(5..3 => {
            count += 1;
        });
        assert_eq!(count, 12);
    }

    #[test]
    fn a_non_numeric_bound_falls_back_to_twelve_repetitions() {
        let mut count = 0_u32;
        internal_repeat!(1..=abc => {
            count += 1;
        });
        assert_eq!(count, 12);
    }

    #[test]
    fn the_current_number_is_written_per_repetition() {
        let mut sum = 0_usize;
        internal_repeat!(1..=3 => {
            sum += $;
        });
        assert_eq!(sum, 6);
    }

    #[test]
    fn the_caret_placeholders_write_the_range_bounds() {
        let mut sum = 0_usize;
        internal_repeat!(1..=4 => {
            sum += ^$;
        });
        assert_eq!(sum, 16);

        let mut sum = 0_usize;
        internal_repeat!(1..=4 => {
            sum += $^;
        });
        assert_eq!(sum, 4);
    }

    #[test]
    fn the_plus_and_minus_placeholders_shift_the_current_number() {
        let mut sum = 0_usize;
        internal_repeat!(1..=3 => {
            sum += $+;
        });
        assert_eq!(sum, 2 + 3 + 4);

        let mut sum = 0_usize;
        internal_repeat!(1..=3 => {
            sum += $-;
        });
        // `$-` clamps at the range's start: 1-1, 2-1, 3-1.
        assert_eq!(sum, 3);
    }

    #[test]
    fn an_identifier_gains_the_current_number_as_a_suffix() {
        let mut sum = 0_usize;
        internal_repeat!(1..=3 => {
            let value$ = $;
            sum += value$;
        });
        assert_eq!(sum, 6);
    }

    #[test]
    fn an_identifier_suffix_survives_the_number_being_inserted() {
        let mut sum = 0_usize;
        internal_repeat!(1..=3 => {
            let value$arg = $;
            sum += value$arg;
        });
        assert_eq!(sum, 6);
    }

    #[test]
    fn an_identifier_can_take_a_shifted_or_minimum_number() {
        let mut sum = 0_usize;
        internal_repeat!(1..=3 => {
            let next$+ = $;
            sum += next$+;
        });
        assert_eq!(sum, 6);

        let mut sum = 0_usize;
        internal_repeat!(1..=3 => {
            let previous$- = $;
            sum += previous$-;
        });
        assert_eq!(sum, 6);

        // `ident$^` names the range's start, so each repetition shadows the last.
        let mut sum = 0_usize;
        internal_repeat!(1..=3 => {
            let first$^ = $;
            sum += first$^;
        });
        assert_eq!(sum, 6);
    }

    #[test]
    fn a_comma_separated_group_repeats_as_a_tuple() {
        let tuple: (usize, usize, usize) = internal_repeat!(3..=3 => {
            ( ( $, +) )
        });
        assert_eq!(tuple, (1, 2, 3));
    }

    #[test]
    fn a_semicolon_separated_group_repeats_as_statements() {
        let mut count = 0_u32;
        internal_repeat!(3..=3 => {
            ( count += 1; +)
        });
        assert_eq!(count, 3);
    }

    #[test]
    fn a_group_without_a_separator_repeats_what_it_holds() {
        let mut sum = 0_usize;
        internal_repeat!(3..=3 => {
            ( { sum += $; } +)
        });
        assert_eq!(sum, 6);
    }

    #[test]
    fn a_bare_marker_repeats_nothing() {
        let mut count = 0_u32;
        internal_repeat!(3..=3 => {
            count += 1;
            (+)
        });
        assert_eq!(count, 1);
    }

    #[test]
    fn a_trailing_plus_repeats_the_group_once_more() {
        let mut count = 0_u32;
        internal_repeat!(3..=3 => {
            ( count += 1; +)+
        });
        assert_eq!(count, 4);
    }

    #[test]
    fn a_trailing_minus_repeats_the_group_once_less() {
        let mut count = 0_u32;
        internal_repeat!(3..=3 => {
            ( count += 1; +)-
        });
        assert_eq!(count, 2);
    }

    #[test]
    fn a_trailing_caret_repeats_the_complement_of_the_range() {
        let mut count = 0_u32;
        internal_repeat!(2..=4 => {
            ( { count += 1; } +)^
        });
        // The range's end (4) is the current number further from the end at each step: 2 + 1 + 0.
        assert_eq!(count, 3);
    }
}

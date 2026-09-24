//! The Vault's lock, across the C ABI.
//!
//! What locks a Vault is a guard being held, and a guard is a value rather than a name, so what
//! crosses the ABI is a pointer to one. It is opaque in both directions: C takes one from
//! `RolaVault_get_locking_guard` and gives it back to `RolaVault_drop_locking_guard`, and nothing
//! about the guard — what it holds, how it releases the lock, how long it must live — is readable
//! from outside. Holding the pointer is holding the lock, and giving it back is giving the lock
//! back, so a consumer that leaks one leaves the Vault locked.
//!
//! The lock is the Vault's own, at its root: a Vault nested in another locks itself rather than the
//! one holding it, since the Vault a run is in is what it changes.

use std::ptr;

use rorolala_storage::{Lockable, LockingGuard};

use crate::{RolaVault, Vault};

/// A lock on a Vault, as C holds it.
///
/// The guard is a Rust value whose type C has no name for, and this is the name the ABI spells
/// it by: the header declares it opaque, so C never learns what one holds.
///
/// # FFI
/// Opaque: C holds a pointer to one and reaches it only through
/// `RolaVault_get_locking_guard`, which hands one out, and `RolaVault_drop_locking_guard`,
/// which gives the lock back with it.
pub type RolaVaultLocking = LockingGuard<Vault>;

/// Whether the Vault is locked.
///
/// # FFI
///
/// Answers whether the Vault's lock file is there. A Vault locked by another run and one whose lock
/// was left behind by a run that did not finish are both answered as locked, since a lock is the
/// file and not a claim about who is holding it.
///
/// # Safety
///
/// `vault` must be a pointer `locate_rola_vault` handed out, and its owner must not have released
/// it.
#[allow(nonstandard_style)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn RolaVault_is_locking(vault: *const RolaVault) -> bool {
    // SAFETY: the caller keeps the handle from `locate_rola_vault` alive for the call, which is all
    // this reads it over.
    let Some(vault) = (unsafe { vault.as_ref() }) else {
        return false;
    };

    let vault: &Vault = &vault.0;

    vault.is_locking()
}

/// Locks the Vault, handing back the guard the lock is held by.
///
/// # FFI
///
/// Returns a pointer the caller releases with `RolaVault_drop_locking_guard`, and null when nothing
/// was locked. The lock is held until then — a caller that keeps the pointer keeps the lock,
/// whatever else it does.
///
/// A Vault that is locked already is not waited for: nothing is handed back and it is the caller's
/// to decide what to do about it. Whether it was locked already or its lock file could not be made
/// is not told here; `RolaVault_is_locking` is what says which.
///
/// # Safety
///
/// As [`RolaVault_is_locking`]: the pointer must be one `locate_rola_vault` handed out and not yet
/// released.
#[allow(nonstandard_style)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn RolaVault_get_locking_guard(
    vault: *const RolaVault,
) -> *mut RolaVaultLocking {
    // SAFETY: as `RolaVault_is_locking`.
    let Some(vault) = (unsafe { vault.as_ref() }) else {
        return ptr::null_mut();
    };

    let vault: &Vault = &vault.0;

    // A lock is taken asynchronously and this cannot be, so the two meet here: a runtime of this
    // call's own waits for the lock, which is the same meeting `action_handshake` makes.
    let Ok(runtime) = tokio::runtime::Runtime::new() else {
        return ptr::null_mut();
    };

    runtime
        .block_on(vault.lock())
        .map_or(ptr::null_mut(), |guard| Box::into_raw(Box::new(guard)))
}

/// Releases a guard `RolaVault_get_locking_guard` handed out, unlocking the Vault.
///
/// # FFI
///
/// Frees the pointer and gives the lock back with it, so the Vault is unlocked by the time this
/// returns. A null pointer is nothing to release and does nothing.
///
/// # Safety
///
/// `guard` must be a pointer from `RolaVault_get_locking_guard` that has not been released, or null.
#[allow(nonstandard_style)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn RolaVault_drop_locking_guard(guard: *mut RolaVaultLocking) {
    if guard.is_null() {
        return;
    }

    // SAFETY: the caller guarantees the pointer came from `RolaVault_get_locking_guard` and has not
    // been released, which is what taking the box back takes over. Dropping the guard is what gives
    // the lock back.
    drop(unsafe { Box::from_raw(guard) });
}

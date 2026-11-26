//! The Vault's lock, across the C ABI.
//!
//! What locks a Vault is a guard being held, and a guard is a value rather than a name, so what
//! crosses the ABI is a pointer to one. It is opaque in both directions: C takes one from
//! `rola_vault_lock_acquire` and gives it back to `rola_vault_lock_release`, and nothing
//! about the guard (what it holds, how it releases the lock, how long it must live) is readable
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
/// `rola_vault_lock_acquire`, which hands one out, and `rola_vault_lock_release`,
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
/// `vault` must be a pointer `rola_vault_locate` handed out, and its owner must not have released
/// it.
// The name is the C header's spelling: the exported type, an underscore, then the action. That is
// what the generated header declares and what a C caller links against, not a Rust identifier style.
#[allow(nonstandard_style)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rola_vault_is_locking(vault: *const RolaVault) -> bool {
    // SAFETY: the caller keeps the handle from `rola_vault_locate` alive for the call, which is all
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
/// Returns a pointer the caller releases with `rola_vault_lock_release`, and null when nothing
/// was locked. The lock is held until then, a caller that keeps the pointer keeps the lock,
/// whatever else it does.
///
/// A Vault that is locked already is not waited for: nothing is handed back and it is the caller's
/// to decide what to do about it. Whether it was locked already or its lock file could not be made
/// is not told here; `rola_vault_is_locking` is what says which.
///
/// # Safety
///
/// As [`rola_vault_is_locking`]: the pointer must be one `rola_vault_locate` handed out and not yet
/// released.
// The name is the C header's spelling: the exported type, an underscore, then the action. That is
// what the generated header declares and what a C caller links against, not a Rust identifier style.
#[allow(nonstandard_style)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rola_vault_lock_acquire(vault: *const RolaVault) -> *mut RolaVaultLocking {
    // SAFETY: as `rola_vault_is_locking`.
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

/// Releases a guard `rola_vault_lock_acquire` handed out, unlocking the Vault.
///
/// # FFI
///
/// Frees the pointer and gives the lock back with it, so the Vault is unlocked by the time this
/// returns. A null pointer is nothing to release and does nothing.
///
/// # Safety
///
/// `guard` must be a pointer from `rola_vault_lock_acquire` that has not been released, or null.
// The name is the C header's spelling: the exported type, an underscore, then the action. That is
// what the generated header declares and what a C caller links against, not a Rust identifier style.
#[allow(nonstandard_style)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rola_vault_lock_release(guard: *mut RolaVaultLocking) {
    if guard.is_null() {
        return;
    }

    // SAFETY: the caller guarantees the pointer came from `rola_vault_lock_acquire` and has not
    // been released, which is what taking the box back takes over. Dropping the guard is what gives
    // the lock back.
    drop(unsafe { Box::from_raw(guard) });
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::ptr;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::{RolaVault, Vault};

    use super::{
        RolaVaultLocking, rola_vault_is_locking, rola_vault_lock_acquire, rola_vault_lock_release,
    };

    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rola-vault-ffi-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        dir
    }

    #[test]
    fn a_vault_lock_is_taken_and_given_back() {
        let dir = scratch("lock");
        let vault = RolaVault(Vault::at(dir.clone()));

        // SAFETY: the handle is live for the call.
        assert!(!unsafe { rola_vault_is_locking(&raw const vault) });

        // SAFETY: as above.
        let guard: *mut RolaVaultLocking = unsafe { rola_vault_lock_acquire(&raw const vault) };
        assert!(!guard.is_null());
        // SAFETY: as above.
        assert!(unsafe { rola_vault_is_locking(&raw const vault) });

        // SAFETY: the pointer came from the acquire above and has not been released.
        unsafe { rola_vault_lock_release(guard) };
        // SAFETY: as above.
        assert!(!unsafe { rola_vault_is_locking(&raw const vault) });

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_null_handle_is_not_locked() {
        // SAFETY: null is what the exports accept as nothing to act on.
        assert!(!unsafe { rola_vault_is_locking(ptr::null()) });
        // SAFETY: as above.
        assert!(unsafe { rola_vault_lock_acquire(ptr::null()) }.is_null());
        // SAFETY: as above.
        unsafe { rola_vault_lock_release(ptr::null_mut()) };
    }
}

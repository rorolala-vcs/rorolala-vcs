#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

use rorolala_utils_lazyffi::lazyffi;

/// Authentication
pub use rorolala_auth as auth;

/// Transport and wire protocol
pub use rorolala_protocol as protocol;

/// Client-side workspace
pub use rorolala_workspace as workspace;

/// Server-side vault
pub use rorolala_vault as vault;

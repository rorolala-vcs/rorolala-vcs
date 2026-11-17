//! What a write calls the work while it is doing it.
//!
//! The library does not know the caller's language, and a progress bar whose words are English in a
//! Chinese program is a bar that looks broken. The caller fills this from its own translations; the
//! library only chooses which word a step is.

/// The words a write uses while it runs.
#[derive(Debug, Clone)]
pub struct Labels {
    /// Reading the tree, a Layout or an index.
    pub reading: String,
    /// Writing content into the store.
    pub writing: String,
    /// Reaching the Vault for a Layout or an object.
    pub fetching: String,
    /// Carrying content to the Vault.
    pub uploading: String,
    /// Carrying content from the Vault.
    pub downloading: String,
    /// Checking the work before writing it.
    pub checking: String,
    /// Carrying out a plan.
    pub applying: String,
}

impl Default for Labels {
    fn default() -> Self {
        Self {
            reading: "Reading".to_owned(),
            writing: "Writing".to_owned(),
            fetching: "Fetching".to_owned(),
            uploading: "Uploading".to_owned(),
            downloading: "Downloading".to_owned(),
            checking: "Checking".to_owned(),
            applying: "Applying".to_owned(),
        }
    }
}

use rorolala_utils_configure::Configure;
use rorolala_utils_lazyffi::lazyffi;
use serde::{Deserialize, Serialize};

/// Top-level configuration for the Workspace
///
/// A Workspace keeps its own settings beside the data it holds. It keeps none of them
/// yet: the file is written when the Workspace is created, so that there is one to read
/// and to add to, and it says nothing until there is something to say.
#[lazyffi(export = WorkspaceConfig)]
#[derive(Debug, Default, Clone, Configure, Serialize, Deserialize)]
pub struct Config {}

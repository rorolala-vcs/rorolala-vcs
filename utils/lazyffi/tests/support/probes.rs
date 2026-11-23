// The input exports the proof in `input.rs` compiles and runs.
//
// The file is included rather than built as a test of its own, and it lives under
// `tests/` rather than beside the library, so the workspace's header generator never
// sees these probes: they exist to prove the input shapes, not to join the shipping
// surface.

use rorolala_utils_lazyffi::lazyffi;

/// What a caller asks about.
#[lazyffi(export = RolaProbeRequest, input)]
pub struct ProbeRequest {
    /// The workspace to look in.
    pub workspace: String,
    /// The account asking.
    pub account: String,
    /// The files to ask about.
    pub files: rorolala_utils_lazyffi::StrSpan,
    /// Whether to stay off the network.
    pub offline: bool,
    /// Whether to redo work already done.
    pub force: bool,
}

/// One change a caller asks for.
#[lazyffi(export = RolaProbeOp)]
pub enum ProbeOp {
    /// Adds an entry.
    CreateEntry {
        /// The path to add.
        path: String,
    },
    /// Removes an entry.
    RemoveEntry {
        /// The uuid to remove.
        uuid: String,
    },
}

/// Why a probe refused.
#[lazyffi(export = RolaProbeError)]
pub enum ProbeError {
    /// There was nothing to do.
    Nothing,
}

/// Answers a request, describing what crossed.
#[lazyffi(export = rola_probe_request)]
pub fn probe_request(request: &ProbeRequest) -> Result<String, ProbeError> {
    Ok(format!(
        "{}|{}|{}|{}|{}",
        request.workspace,
        request.account,
        request.files.as_slice().join(","),
        request.offline,
        request.force,
    ))
}

/// Describes the ops that crossed.
#[lazyffi(export = rola_probe_ops)]
pub fn probe_ops(ops: &[ProbeOp]) -> Result<String, ProbeError> {
    let described = ops
        .iter()
        .map(|op| match op {
            ProbeOp::CreateEntry { path } => format!("create:{path}"),
            ProbeOp::RemoveEntry { uuid } => format!("remove:{uuid}"),
        })
        .collect::<Vec<_>>()
        .join(";");

    Ok(described)
}

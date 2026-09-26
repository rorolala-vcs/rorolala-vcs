#Requires -Version 5.1
# Lints the C# side of the workspace: every rule the analyzer set carries, over the whole solution,
# with any diagnostic failing the run.
#
# This is the counterpart of `clippy` for the Rust side, and it is a script of its own for the same
# reason clippy is: the extra tier — `AnalysisMode=All`, which is every rule rather than the
# recommended few — is asked for here rather than in `Directory.Build.props`, so that an ordinary
# build is held to the warnings the compiler already raises while this gate is held to the whole of
# what the analyzers say. What does not fit an application is excused one rule at a time, with the
# reason, in the root `.editorconfig`; the tier is not lowered for it.
#
# Two things here are load-bearing rather than incidental:
#
#   * `TreatWarningsAsErrors` is turned *off* and MSBuild's own `-warnAsError` turned *on*. The
#     compiler's switch stops the solution at the first project that fails, so one rule broken in
#     three programs would take three runs to find; MSBuild's promotes the same warnings to errors
#     and carries on, so every one is reported in one run and the run still fails.
#   * `-t:Rebuild` (a full rebuild) rather than a plain build. The analyzers are the point of this
#     gate, and a project MSBuild calls up to date is one nothing analyzes — which a change to
#     `.editorconfig` or `Directory.Build.props` does not by itself undo.
$ErrorActionPreference = 'Stop'

. "$PSScriptRoot/../lib/common.ps1"

& $DotnetProgram @DotnetFlags msbuild $Solution -restore -t:Rebuild -warnAsError -nologo -v:m -tl:off `
	-p:Configuration=Release `
	-p:AnalysisMode=All `
	-p:AnalysisLevel=latest-all `
	-p:EnforceCodeStyleInBuild=true `
	-p:TreatWarningsAsErrors=false
Assert-Exit

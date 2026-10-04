#Requires -Version 5.1
# The Desktop program on its own, laid out where an export lays it out, with the plugins that ship
# with it beside it.
#
# The C ABI is built first, and deliberately: a plugin that answers who holds an entry binds the
# library, and both the library and the header the bindings are generated from are the Rust side's
# output. Building the Desktop without them would be building against the last ones there happened to
# be, or failing at the header.
$ErrorActionPreference = 'Stop'

. "$PSScriptRoot/../lib/common.ps1"

Invoke-Script lib

Publish-Desktop $DesktopDir
Publish-Plugins

#pragma once

// The generated layer comes first: the hand-written headers name its handles,
// aliases and constants.
#include <rola_plus_plus/gen/rola_plus_plus.gen.hpp>

#include "detail.hpp"
#include "error.hpp"
#include "error_map.hpp"
#include "layout.hpp"
#include "status.hpp"
#include "storage.hpp"
#include "tree.hpp"
#include "variant_graph.hpp"
#include "vcs_index.hpp"
#include "workspace.hpp"
#include "vault.hpp"
#include "account.hpp"

// The write side comes after the readings it names. The request and result
// records are values; the writers only see a reading.
#include "write.hpp"
#include "write_request.hpp"
#include "write_result.hpp"
#include "layout_writer.hpp"
#include "storage_writer.hpp"
#include "tree_writer.hpp"
#include "vcs_index_writer.hpp"
#include "vault_writer.hpp"
#include "workspace_writer.hpp"
#include "daemon_requester.hpp"

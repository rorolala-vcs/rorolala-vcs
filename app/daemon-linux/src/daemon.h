#pragma once

#include <glib.h>
#include <stddef.h>

#include "process.h"

/* Where the daemon's output goes. The window sets this before it starts the
   daemon; every chunk arrives on the main loop as UTF-8. */
typedef void (*DaemonOutputFunc)(const char *text);

void daemon_set_output_func(DaemonOutputFunc func);

gboolean daemon_start(const char *exe, const CmdLine *arguments, char *error,
                      size_t error_cap);
void daemon_stop(void);
gboolean daemon_is_running(void);
int daemon_pid(void);
int daemon_last_exit_code(void);

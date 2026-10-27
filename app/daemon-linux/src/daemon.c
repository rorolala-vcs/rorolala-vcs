#include "daemon.h"

#include <errno.h>
#include <signal.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

#include "util.h"

/* One run of the daemon, with the pipe its output arrives on. A run outlives
   the next one: a stop and a start back to back leave the first run draining
   while the second is already writing, and each run owns its own channel. */
typedef struct DaemonRun {
  GPid pid;
  int fd;
  GIOChannel *channel;
  guint watch;
  gboolean exited;
  gboolean closed;
  int exit_code;
} DaemonRun;

static DaemonRun *g_run;
static int g_last_exit_code;
static DaemonOutputFunc g_output;

void daemon_set_output_func(DaemonOutputFunc func) { g_output = func; }

static int exit_code_of(int status) {
  if (WIFEXITED(status)) {
    return WEXITSTATUS(status);
  }
  if (WIFSIGNALED(status)) {
    return 128 + WTERMSIG(status);
  }
  return -1;
}

static void run_maybe_free(DaemonRun *run) {
  if (!run->exited || !run->closed) {
    return;
  }

  if (run->watch) {
    g_source_remove(run->watch);
    run->watch = 0;
  }
  if (run->channel) {
    g_io_channel_unref(run->channel);
    run->channel = NULL;
  }
  if (run->fd >= 0) {
    close(run->fd);
    run->fd = -1;
  }
  if (g_run == run) {
    g_run = NULL;
  }
  g_free(run);
}

static gboolean daemon_output_ready(GIOChannel *source, GIOCondition condition,
                                    gpointer user_data) {
  DaemonRun *run = (DaemonRun *)user_data;
  char buffer[4097];

  (void)condition;

  for (;;) {
    gsize read = 0;
    GError *error = NULL;
    GIOStatus status = g_io_channel_read_chars(
        source, buffer, sizeof(buffer) - 1, &read, &error);

    if (status == G_IO_STATUS_NORMAL) {
      g_clear_error(&error);
      if (read == 0) {
        return G_SOURCE_CONTINUE;
      }
      buffer[read] = '\0';
      if (g_output) {
        g_output(buffer);
      }
      continue;
    }

    if (status == G_IO_STATUS_AGAIN) {
      g_clear_error(&error);
      return G_SOURCE_CONTINUE;
    }

    /* The daemon closed its write end, or the pipe failed: either way there is
       nothing more to read from this run. */
    g_clear_error(&error);
    run->closed = TRUE;
    run->watch = 0;
    if (run->channel) {
      g_io_channel_unref(run->channel);
      run->channel = NULL;
    }
    if (run->fd >= 0) {
      close(run->fd);
      run->fd = -1;
    }
    run_maybe_free(run);
    return G_SOURCE_REMOVE;
  }
}

/* See the twin in `process.c`: one pipe carries both streams. */
static void merge_stderr(gpointer user_data) {
  (void)user_data;
  (void)dup2(STDOUT_FILENO, STDERR_FILENO);
}

static void daemon_refresh(void) {
  DaemonRun *run = g_run;
  int status = 0;
  pid_t result;

  if (!run || run->exited) {
    return;
  }

  result = waitpid(run->pid, &status, WNOHANG);
  if (result == 0) {
    return;
  }

  run->exited = TRUE;
  run->exit_code = result == run->pid ? exit_code_of(status) : -1;
  run->pid = 0;
  g_last_exit_code = run->exit_code;
  run_maybe_free(run);
}

gboolean daemon_start(const char *exe, const CmdLine *arguments, char *error,
                      size_t error_cap) {
  char **argv;
  gsize count;
  gsize index;
  char *working_dir;
  GPid pid = 0;
  int input_fd = -1;
  int output_fd = -1;
  int error_fd = -1;
  GError *gerror = NULL;
  gboolean started;
  DaemonRun *run;

  if (error && error_cap) {
    error[0] = '\0';
  }

  if (daemon_is_running()) {
    if (error && error_cap) {
      str_copy(error, error_cap, "The daemon is already running.");
    }
    return FALSE;
  }

  if (!exe || !exe[0] || !path_exists(exe)) {
    if (error && error_cap) {
      str_copy(error, error_cap, "The rola-daemon executable was not found.");
    }
    return FALSE;
  }

  count = arguments && arguments->args ? arguments->args->len : 0;
  argv = g_new0(char *, count + 2);
  /* As in `run_process`: the words are read, never written. */
  argv[0] = (char *)exe;
  for (index = 0; index < count; ++index) {
    argv[index + 1] = g_ptr_array_index(arguments->args, index);
  }
  argv[count + 1] = NULL;

  working_dir = path_dirname(exe);
  started = g_spawn_async_with_pipes(
      working_dir, argv, NULL, G_SPAWN_DO_NOT_REAP_CHILD | G_SPAWN_SEARCH_PATH,
      merge_stderr, NULL, &pid, &input_fd, &output_fd, &error_fd, &gerror);
  g_free(argv);
  g_free(working_dir);

  if (!started) {
    if (error && error_cap) {
      char *detail = g_strdup_printf("Could not start rola-daemon: %s",
                                     gerror ? gerror->message
                                            : "the program could not be run.");
      str_copy(error, error_cap, detail);
      g_free(detail);
    }
    g_clear_error(&gerror);
    return FALSE;
  }

  /* The daemon has no use for stdin, and closing the parent's end is what says
     so; the error stream was merged into the output stream in the child. */
  close(input_fd);
  close(error_fd);

  run = g_new0(DaemonRun, 1);
  run->pid = pid;
  run->fd = output_fd;
  run->exit_code = -1;
  run->channel = g_io_channel_unix_new(output_fd);
  /* UTF-8 is what the programs write, and letting the channel decode it keeps
     a character split across two reads from reaching the text buffer broken. */
  (void)g_io_channel_set_encoding(run->channel, "UTF-8", NULL);
  (void)g_io_channel_set_flags(run->channel, G_IO_FLAG_NONBLOCK, NULL);
  run->watch = g_io_add_watch(run->channel, G_IO_IN | G_IO_HUP | G_IO_ERR,
                              daemon_output_ready, run);

  g_run = run;
  g_last_exit_code = 0;
  return TRUE;
}

void daemon_stop(void) {
  DaemonRun *run;
  pid_t pid;
  int status = 0;
  gboolean reaped = FALSE;
  gboolean status_valid = FALSE;
  int attempt;

  daemon_refresh();
  run = g_run;
  if (!run || run->exited) {
    return;
  }

  pid = run->pid;

  /* rola-daemon has no stop command, so stopping the process this window
     started is the whole of it — the same thing `TerminateProcess` is on
     Windows. SIGTERM is asked first so the runtime may shut down cleanly;
     SIGKILL is what is left when it will not. */
  (void)kill(pid, SIGTERM);

  for (attempt = 0; attempt < 100; ++attempt) {
    pid_t result = waitpid(pid, &status, WNOHANG);

    if (result == pid) {
      reaped = TRUE;
      status_valid = TRUE;
      break;
    }
    if (result < 0) {
      if (errno == ECHILD) {
        reaped = TRUE;
      }
      if (errno != EINTR) {
        break;
      }
    }
    g_usleep(50 * 1000);
  }

  if (!reaped) {
    (void)kill(pid, SIGKILL);
    for (;;) {
      pid_t result = waitpid(pid, &status, 0);

      if (result == pid) {
        status_valid = TRUE;
        break;
      }
      if (result < 0 && errno == EINTR) {
        continue;
      }
      break;
    }
  }

  run->exited = TRUE;
  run->exit_code = status_valid ? exit_code_of(status) : -1;
  run->pid = 0;
  g_last_exit_code = run->exit_code;
  run_maybe_free(run);
}

gboolean daemon_is_running(void) {
  daemon_refresh();
  return g_run && !g_run->exited;
}

int daemon_pid(void) {
  daemon_refresh();
  return (g_run && !g_run->exited) ? (int)g_run->pid : 0;
}

int daemon_last_exit_code(void) { return g_last_exit_code; }

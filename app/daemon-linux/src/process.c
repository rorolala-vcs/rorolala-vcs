#include "process.h"

#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

#include "util.h"

void cmd_init(CmdLine *cmd) {
  cmd->args = g_ptr_array_new_with_free_func(g_free);
}

void cmd_free(CmdLine *cmd) {
  if (cmd->args) {
    g_ptr_array_unref(cmd->args);
    cmd->args = NULL;
  }
}

gboolean cmd_add(CmdLine *cmd, const char *argument) {
  if (!cmd->args) {
    return FALSE;
  }
  g_ptr_array_add(cmd->args, g_strdup(argument ? argument : ""));
  return TRUE;
}

void process_result_init(ProcessResult *result) {
  result->exit_code = -1;
  result->output = NULL;
  result->output_len = 0;
}

void process_result_free(ProcessResult *result) {
  g_free(result->output);
  process_result_init(result);
}

/* The child is given one pipe for both streams, which is what the Windows twin
   captures: writing stderr where stdout goes is the whole of the merge, and it
   runs in the child before the program is replaced. */
static void merge_stderr(gpointer user_data) {
  (void)user_data;
  (void)dup2(STDOUT_FILENO, STDERR_FILENO);
}

gboolean run_process(const char *exe_path, const CmdLine *cmd,
                     const char *working_dir, ProcessResult *result,
                     char *error, size_t error_cap) {
  char **argv;
  gsize count;
  gsize index;
  char *output = NULL;
  char *errors = NULL;
  gint status = 0;
  GError *gerror = NULL;
  gboolean ok;

  process_result_init(result);
  if (error && error_cap) {
    error[0] = '\0';
  }

  if (!exe_path || !exe_path[0]) {
    if (error && error_cap) {
      str_copy(error, error_cap, "No executable was named.");
    }
    return FALSE;
  }

  count = cmd && cmd->args ? cmd->args->len : 0;
  argv = g_new0(char *, count + 2);
  /* g_spawn only reads the words; the cast is the array's type, not a promise
     that anything here will be written. */
  argv[0] = (char *)exe_path;
  for (index = 0; index < count; ++index) {
    argv[index + 1] = g_ptr_array_index(cmd->args, index);
  }
  argv[count + 1] = NULL;

  ok = g_spawn_sync((working_dir && working_dir[0]) ? working_dir : NULL, argv,
                    NULL, G_SPAWN_SEARCH_PATH, merge_stderr, NULL, &output,
                    &errors, &status, &gerror);
  g_free(argv);
  g_free(errors);

  if (!ok) {
    if (error && error_cap) {
      str_copy(error, error_cap,
               gerror ? gerror->message : "The program could not be started.");
    }
    g_clear_error(&gerror);
    g_free(output);
    return FALSE;
  }

  result->exit_code = WIFEXITED(status)
                          ? WEXITSTATUS(status)
                          : 128 + (WIFSIGNALED(status) ? WTERMSIG(status) : 0);
  result->output = output ? output : g_strdup("");
  result->output_len = output ? strlen(output) : 0;
  return TRUE;
}

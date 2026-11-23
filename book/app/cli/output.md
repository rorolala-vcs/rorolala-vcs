---
title: "Output Modes"
description: "How --json and --json-pretty write a result as data, how --format writes it through a MiniJinja template, and which one is used when more than one is named."
keywords: output, format, json, json-pretty, minijinja, template, rows, columns, stdout, query
---

# Output Modes

A command that finds something can say it in three ways. It can draw the human form it was designed around, write the same result as JSON, or draw it through a template of your own. What is drawn goes to standard output; progress and errors stay on standard error.

## The flags

| Flag                  | What is written                                         |
| --------------------- | ------------------------------------------------------- |
| none                  | The command's own drawing: rows, columns, or a sentence |
| `--json`              | The result as one compact JSON value                    |
| `--json-pretty`       | The same value, indented                                |
| `--format <TEMPLATE>` | The result drawn through a MiniJinja template           |

`--json`, `--json-pretty` and `--format` are alternatives to the human drawing, not additions to it. A run uses one of them.

They do not reach every command equally. `--json` and `--json-pretty` write the result of every command that has a result to serialize. `--format` replaces a drawing that is made of rows or a reading; a command that answers in one line of its own keeps that line even when `--format` is named.

## The data is the same

Where a command has both, they read one result. The names a template uses are the names `--json` writes, so a template and a JSON reader say the same things about the same fields.

The practical order is therefore:

1. Run the command with `--json`.
2. Read the field names in the result.
3. Write a template with those names.

A name that is not in the result is an error rather than an empty value, so a typo in a template is seen rather than silently printing nothing.

## JSON

```sh
rola account ls --json
```

```json
{ "accounts": [{ "name": "alice", "is_current": true }] }
```

`--json-pretty` writes the same value with indentation:

```json
{
  "accounts": [
    {
      "name": "alice",
      "is_current": true
    }
  ]
}
```

A result is one JSON value: an object, an array, or a scalar. A command that only reports an action may have no data to write.

When `--json` or `--json-pretty` is used, progress is written as one JSON record per line on standard error instead of an interactive bar. The run is being read by a program rather than watched by a person. `--no-progress` silences the records as it silences the bar.

## Templates

`--format` takes a MiniJinja template. The template is rendered from the result and printed to standard output in place of the human drawing.

### One list

Most query commands have one list to show: accounts, keys, entries, Layouts. The template is rendered once per item, with the list's name in the context.

```sh
rola account ls --format "{{ accounts.name }}"
rola account ls --format "{{ accounts.name }}{% if accounts.is_current %} (current){% endif %}"
rola key --format "{{ paths }}"
```

`{{ accounts.name }}` prints one line per account. `{{ paths }}` prints one line per key. A rendering that does not end in a newline is given one, so each item is a line of its own.

### Several lists

A reading whose parts are lists of their own binds every name at once and renders the template once. Every name is then an array, so a template names a whole list rather than one row.

```sh
rola layout tree-diff --format "lost {{ lost | length }}, untagged {{ untagged | length }}"
rola layout tree-diff --format "{{ lost | join('\n') }}"
```

The reading has the lists `lost`, `untagged`, `modified`, `renamed`, `failed`, `trees_modified` and `wrong`. Filters and loops are MiniJinja's, so `join`, `length`, `sort` and `{% for %}` work as they do anywhere else.

### Which shape a command has

If `--json` prints an array under one name, the command is the one-list shape and its template is drawn per item. If it prints an object whose fields are arrays, it is the several-list shape and its template is drawn once. The JSON is the answer for both, which is why it is the place to start.

## Which one wins

`--json` and `--json-pretty` come first. Naming one of them with `--format` prints the JSON, ignores the template, and says so on standard error:

```text
--format is ignored here: --json and its kind come first
```

Naming both JSON flags prints the pretty form.

With none of them, the command draws its own form, which is what a person reading a terminal sees.

## A failed run

A template draws a result. A run that fails has no result, so its error is printed as usual, in the same words it uses without `--format`. `--json` does not turn an error into JSON either: an error is a reason, not a value of the command's result.

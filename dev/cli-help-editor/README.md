# rorolala-dev-cli-help-editor

## Introduction

A local editor for the CLI's copy. It lists every help page, error, hint, result, description and other message under the CLI i18n trees (`app/cli/*/i18n`), edits the YAML in place, and previews the text through a port of `utils/cli-theme`'s `TextRendering::render`. A draft entry sits at the top of the list, always, and its text is kept in `__draft.yml` beside the editor rather than in any tree: it is where a message is written before it belongs to a command. The gear beside Save holds the OpenAI compatible endpoint the Translate button streams from: it reads and writes the data home's `rola/dev/llm-tools.toml`, and the button names the language it writes into, which is the language the open tab shows. It runs on Node.js 20 or newer and is reached as `cargo dev cli-help-editor`.

## Boundary

It owns the browser UI, the source position reader and writer for the i18n subset, and the preview renderer used while editing. It does not own the wording of the translation files, the Rust renderer, or what the command line prints. The preview mirrors `utils/cli-theme` and is checked against its cases; the program still decides the final output. The endpoint file is rewritten in its `[translation]` section alone, so other tables and comments in it are kept.

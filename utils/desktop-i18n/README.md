# RorolalaDesktopI18n

## Introduction

Reads the `rust-i18n` translation files the command line keeps and answers the Desktop program's lookups in the locale it was given.

## Boundary

It owns reading the YAML files, registering directories in the order that decides precedence, and resolving a key with its values. It does not author translations, which are written under `app/cli/rola/i18n/` in `en` and `zh-CN`, and it does not draw UI: the desktop program under `app/desktop` draws what it is handed.

# RorolalaDesktopI18n

The Desktop program's translations, read from the files the command line keeps.

The command line speaks through `rust-i18n`; its words live in YAML under `app/cli/rola/i18n`,
one file per command or group of errors, each node a key and each leaf that key written in every
language it has been written in. What the window says is written the same way, into files of the
same shape, so that a person writing either one writes the other without a second set of rules to
hold in mind — and so that a file stating words both of them say can be read by both.

Nothing here is a second format. A file that `rust-i18n` reads is a file this reads.

## The format

```yaml
_version: 2

error:
  vault:
    err_not_bound:
      en: "`%{name}` is not bound to a Vault"
      zh-CN: "`%{name}` 未绑定到任何保险库"
```

- A file opens with `_version`, which is not a node and is not read.
- A node is a mapping of locale to form, which is a mapping whose every value is written out.
- A key is where a node sits in that nesting — `error.vault.err_not_bound`. Which file a node is
  written in, and which directory that file sits under, says nothing about the key: the files under
  one directory are read together, so where a thing is filed is for whoever is filing it.
- Every `*.yml` and `*.yaml` under the directory is read, however deep it sits.
- A form leaves a place for a value as `%{name}`. The name is for whoever reads the file; values go
  in by order, the first value into the first place.

## The API

```csharp
RolaI18N.SetTranslationDirectory(Path.Combine(AppContext.BaseDirectory, "i18n"));
RolaI18N.RegisterTranslationDirectory(pluginDirectory);
RolaI18N.SetLocale(Program.Language ?? "en");

var title = RolaI18N.Get("pack.result_packed");
var refused = RolaI18N.Get("error.vault.err_not_bound", name);
var explained = RolaI18N.Get("explain_exit_code.result", 94, meaning);
```

`SetTranslationDirectory` names one directory on its own; `RegisterTranslationDirectory` adds one
after those already named, and the directories are read in registration order. **First registration
wins**: a key an earlier directory states is not replaced by a later one, so the host registers its
own directory before the plugins register theirs and a plugin cannot overwrite what the host says.
`RegisterTranslationDirectory` ignores a directory registered twice.

`SetTranslationDirectory`, `RegisterTranslationDirectory` and `SetLocale` record what they are given
and can be read back from `RolaI18N.TranslationDirectory` (the first directory),
`RolaI18N.TranslationDirectories` (all of them, in order) and `RolaI18N.Locale`. The files are read
the first time a form is asked for, not when a directory is named, so a program may name its
directories and its language before anything is drawn.

`Get` takes a key and up to twelve values, and has one overload for each count.

## What it decides, and why

- **A key nothing states reads as the key.** So does a key written in no language the program
  speaks. What is wrong is then on the screen rather than hidden behind a blank, and finding it is
  reading the key and looking for it — which is what `rust-i18n` does too.
- **A language with no form falls back to `en`**, the language the command line's own files fall
  back to.
- **A form is trimmed.** A form written as a `|` block ends with a newline that belongs to the file
  rather than to what it says, and the command line trims every form it speaks, so the two say the
  same thing.
- **Values go in by order, and only by order.** A place left without a value is left as it was
  written; a value left without a place is dropped. The form is what says how many values there
  are.
- **A value is written the same in every language.** Numbers are written the way `rust-i18n` writes
  them — without consulting the machine's language — so what a form says does not change with where
  it is read.
- **A language is named as a locale.** `zh-CN`, `zh_CN` and `zh_CN.UTF-8` are the same language, and
  the last two are read as the first.

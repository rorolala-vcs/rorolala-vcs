# rorolala-dev-i18n-check

## Introduction

Scans the tree for every translation key a call site names and reports the ones no translation file states, as the check behind `cargo ci i18n`.

## Boundary

It owns finding the projects, reading their `i18n/` directories as the catalogues, and reading literal keys out of Rust `t!` calls and C# `Get` calls, with the host's keys allowed to a plugin. It does not translate and does not render: the files it reads are the ones `rust-i18n` reads at run time. A key built at run time is counted as unchecked rather than guessed, and `rola-ci` owns running the scan inside the gate.

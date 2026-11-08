# rorolala-utils-configure

## Introduction

A configuration file as a type: read one, edit it in place, and have the edit land atomically by staging it beside the original and renaming it over.

## Boundary

It owns the `Configure` trait, the staging and rename protocol, and the choice of format by file extension. It does not own what a configuration holds or where it lives: the types are declared by the crate that reads them, and the paths under a Workspace or Vault are `rorolala-utils-constants`'.

# rorolala-utils-configure

Configuration files as a type: read one, edit it in place, and have the edit land
atomically.

`Configure` is what a configuration type implements — by deriving it — and `Config<T>`
is the file that holds one:

```rust
use rorolala_utils_configure::{Config, Configure};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Configure)]
struct VaultConfig {
    name: String,
}

fn main() -> Result<(), rorolala_utils_configure::Error> {
    // A file to work on, written through the same trait.
    let path = std::env::temp_dir().join("rorolala-configure-readme.toml");
    VaultConfig { name: "before".to_owned() }.write_to(&path)?;

    {
        let mut config = Config::<VaultConfig>::read(&path)?;
        config.name = "renamed".to_owned();   // through `DerefMut`
        config.write()?;                      // rendered into the staging file
    }   // dropped here, which renames the staging file over the original

    assert_eq!(VaultConfig::read_from(&path)?.name, "renamed");
    Ok(())
}
```

## Why a staging file

Replacing a file in place has two ways to lose it: a process that dies while writing
leaves half a file, and two processes writing leave a mixture of both. So an edit never
touches the original:

1. `read` parses the original and copies it to `<file>.lock` beside it — and `new` stages
   an empty configuration the same way, for a file that is not there yet,
2. `write` renders the contents into that staging file,
3. publishing renames the staging file over the original — one atomic step.

A process that dies mid-edit therefore leaves the original whole and the staged edit
beside it. The next `read` refuses rather than copying over that leftover, so it is
still there to recover or to discard by hand.

## Formats

Chosen by the file's extension: `toml` (and `tml`), `yaml` (and `yml`), and `json` —
which is also what an extension this crate does not know, or none at all, means.

## Failures

Everything fallible reports through `ConfigureError`, an enum that also crosses to C, so a
caller that is not Rust can see what went wrong. Nothing here panics, and nothing
returns an error it did not get from somewhere: the one place a failure cannot be
reported is `Drop`, which publishes too and leaves the staging file behind as the sign
that it did not. `Config::publish` is there for callers that need the error.

//! `layout-remote`: fetching a Vault's Layout, and taking and letting go of an entry.
//!
//! A program rather than a set of tests cargo runs, for the same reason the rest of them are: it
//! makes a place to work in, keeps accounts, binds a Vault, puts an entry in the Vault's Layout,
//! serves the Vault, and then works the commands these exist for.
//!
//! What is checked is that a fetch brings the Vault's Layout here as a copy rather than moving it,
//! that the copy is what the reading commands answer from — including when there is no copy — and
//! that taking and letting go of an entry is written on the Vault and in the copy beside it.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use librorolala::layout::{Layout, LayoutPath, MutableData};
use librorolala::vault::{CONFIG_PATH, KEYS_DIR, LAYOUT_DIR, Vault};
use librorolala::workspace::Workspace;
use rorolala_utils_sandbox::{Guard, Serving, command, run, serve};
use uuid::Uuid;

/// The port the Vault serves on.
const PORT: u16 = 7941;

/// The account the work acts as first, and the one it is handed to after.
const ALICE: &str = "alice";
const BOB: &str = "bob";

/// The name the Workspace knows the Vault by.
const VAULT_NAME: &str = "origin";

/// The entry the Vault's Layout holds, and one it does not.
const ENTRY: Uuid = Uuid::from_u128(0x42);
const GONE: Uuid = Uuid::from_u128(0x99);

/// How long the Vault is waited for before it is given up on.
const STARTUP: Duration = Duration::from_secs(10);

#[tokio::main]
async fn main() {
    let sandbox = Guard::new("layout-remote");
    let data = sandbox.join("data");
    let root = sandbox.join("root");
    let workspace = sandbox.join("ws");

    serveable(&root, PORT);

    Workspace::create(&workspace)
        .unwrap_or_else(|error| panic!("making {}: {error:?}", workspace.display()));
    let auth = workspace.join(".rola").join("auth");
    for name in [ALICE, BOB] {
        pair(&auth, name);
        publish(&auth, &root, name);
    }

    // The Vault's Layout holds one entry, held by nobody. It is written before the Vault is served,
    // so what a fetch brings is what was put there and not a Layout that happened to be empty.
    let vault_layout = Layout::open(root.join(LAYOUT_DIR)).expect("the Vault's Layout");
    vault_layout
        .create_entry(ENTRY, MutableData::new(None, [7; 32], "a file".to_owned()))
        .expect("an entry");
    vault_layout
        .create_path(&LayoutPath::new("a.psd").unwrap(), ENTRY)
        .expect("a path");

    run(&mut client(&workspace, &data, &["account", ALICE])).expect_success();
    run(&mut client(
        &workspace,
        &data,
        &["vault", "bind", VAULT_NAME, &address(PORT)],
    ))
    .expect_success();

    let copy = workspace
        .join(".rola/cache/readonly-layouts")
        .join(VAULT_NAME)
        .join("truth");
    let entry = ENTRY.to_string();
    let gone = GONE.to_string();
    let mut checked = Checked::default();

    // Before anything is fetched there is no copy, and the reading commands say so rather than
    // reaching for a Vault — the Vault is not even serving yet.
    let said = run(&mut client(
        &workspace,
        &data,
        &["layout", "read-ownership", VAULT_NAME, &entry],
    ));
    checked.wants(
        "reading with no copy is refused",
        said.code == Some(192) && said.stderr.contains("has not been fetched"),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    let said = run(&mut client(
        &workspace,
        &data,
        &["layout", "ls-ownership", VAULT_NAME],
    ));
    checked.wants(
        "listing with no copy is refused",
        said.code == Some(192),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    // A Vault's Layout is named `NAME@VAULT`, and reading one that was never fetched is refused the
    // same way whether it is a listing or an entry query.
    let said = run(&mut client(
        &workspace,
        &data,
        &["layout", "entries", "--layout", "truth@origin"],
    ));
    checked.wants(
        "querying an unfetched Vault Layout is refused",
        said.code == Some(192),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    // What is kept under a name is kept under a name: an address is not one.
    let said = run(&mut client(
        &workspace,
        &data,
        &["layout", "fetch", &address(PORT)],
    ));
    checked.wants(
        "fetching by address is refused",
        said.code == Some(71),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    let serving = serve_vault(&root, PORT);

    // A fetch brings the Vault's Layout here. What comes back says so, and what is on disk is a
    // Layout of the Workspace's own that holds the entry the Vault holds.
    let said = run(&mut client(
        &workspace,
        &data,
        &["layout", "fetch", VAULT_NAME],
    ));
    checked.wants(
        "the Vault's Layout is fetched",
        said.success() && said.stdout.contains("readonly-layouts"),
        &format!(
            "it ended with {:?} and said {:?}",
            said.code,
            said.stdout.trim()
        ),
    );

    let copied = Layout::open(&copy).expect("the fetched copy");
    checked.wants(
        "the fetched copy is a Layout holding what the Vault held",
        copied.entry(ENTRY).is_some()
            && copied.id_of(&LayoutPath::new("a.psd").unwrap()) == Some(ENTRY),
        "the copy did not hold the entry the Vault held",
    );

    // The copy is what a name written `NAME@VAULT` reads, so the query commands reach it too.
    let said = run(&mut client(
        &workspace,
        &data,
        &["layout", "entries", "--layout", "truth@origin"],
    ));
    checked.wants(
        "a Vault Layout is queried through NAME@VAULT",
        said.success() && clean(&said.stdout).contains("a.psd") && said.stdout.contains(&entry),
        &format!(
            "it ended with {:?} and said {:?}",
            said.code,
            said.stdout.trim()
        ),
    );

    // Asking after one entry answers whether the Vault has deprecated it: it has not, yet.
    let said = run(&mut client(
        &workspace,
        &data,
        &[
            "layout",
            "entries",
            "--layout",
            "truth@origin",
            "--uuid",
            &entry,
            "--json",
        ],
    ));
    checked.wants(
        "an entry the Vault has not deprecated says so",
        said.success() && said.stdout.contains("\"deprecated\":false"),
        &format!(
            "it ended with {:?} and said {:?}",
            said.code,
            said.stdout.trim()
        ),
    );

    let said = run(&mut client(
        &workspace,
        &data,
        &[
            "layout",
            "entries",
            "--layout",
            "truth@origin",
            "--uuid",
            &gone,
            "--json",
        ],
    ));
    checked.wants(
        "asking after an entry the Layout does not name lists nothing",
        said.success() && said.stdout.contains("\"entries\":[]"),
        &format!(
            "it ended with {:?} and said {:?}",
            said.code,
            said.stdout.trim()
        ),
    );

    // A copy is read, never worked in: a command that changes a Layout refuses one.
    let said = run(&mut client(
        &workspace,
        &data,
        &[
            "layout",
            "path",
            "remove",
            "a.psd",
            "--layout",
            "truth@origin",
        ],
    ));
    checked.wants(
        "a fetched copy is refused by a command that writes",
        said.code == Some(190) && clean(&said.stderr).contains("cannot be changed"),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    // An entry nobody holds reads as nobody's, from the copy.
    let said = run(&mut client(
        &workspace,
        &data,
        &["layout", "read-ownership", VAULT_NAME, &entry],
    ));
    checked.wants(
        "an unheld entry reads as held by nobody",
        said.success() && clean(&said.stdout).contains("held by nobody"),
        &format!(
            "it ended with {:?} and said {:?}",
            said.code,
            said.stdout.trim()
        ),
    );

    // Taking it names this account, on the Vault and in the copy beside it.
    let said = run(&mut client(
        &workspace,
        &data,
        &["layout", "req-ownership", VAULT_NAME, &entry],
    ));
    checked.wants(
        "an unheld entry is taken",
        said.success() && clean(&said.stdout).contains(ALICE),
        &format!(
            "it ended with {:?} and said {:?}",
            said.code,
            said.stdout.trim()
        ),
    );

    checked.wants(
        "the Vault names the account that took the entry",
        vault_owner(&root) == Some(ALICE.to_owned()),
        "the Vault's Layout did not name the taker",
    );
    checked.wants(
        "the copy names the account that took the entry",
        Layout::open(&copy)
            .expect("the fetched copy")
            .entry(ENTRY)
            .and_then(|data| data.owner().map(str::to_owned))
            == Some(ALICE.to_owned()),
        "the copy did not follow the Vault",
    );

    // Another account cannot take what is held, and nothing changes when it tries.
    run(&mut client(&workspace, &data, &["account", BOB])).expect_success();
    let said = run(&mut client(
        &workspace,
        &data,
        &["layout", "req-ownership", VAULT_NAME, &entry],
    ));
    checked.wants(
        "an entry another account holds cannot be taken",
        said.code == Some(193) && clean(&said.stderr).contains(ALICE),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    let said = run(&mut client(
        &workspace,
        &data,
        &["layout", "read-ownership", VAULT_NAME, &entry],
    ));
    checked.wants(
        "the holder is unchanged by a refusal",
        said.success() && clean(&said.stdout).contains(ALICE),
        &format!(
            "it ended with {:?} and said {:?}",
            said.code,
            said.stdout.trim()
        ),
    );

    // What the copy lists is every entry the Vault's Layout names, with its holder.
    let said = run(&mut client(
        &workspace,
        &data,
        &["layout", "ls-ownership", VAULT_NAME, "--json"],
    ));
    checked.wants(
        "listing as a program names every entry and its holder",
        said.success() && said.stdout.contains(&entry) && said.stdout.contains(ALICE),
        &format!(
            "it ended with {:?} and said {:?}",
            said.code,
            said.stdout.trim()
        ),
    );

    // Letting go is the holder's to do, and the Vault and the copy both end up naming nobody.
    run(&mut client(&workspace, &data, &["account", ALICE])).expect_success();
    let said = run(&mut client(
        &workspace,
        &data,
        &["layout", "giveup-ownership", VAULT_NAME, &entry],
    ));
    checked.wants(
        "the holder lets the entry go",
        said.success(),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    checked.wants(
        "the Vault names nobody after a give-up",
        vault_owner(&root).is_none(),
        "the Vault still named a holder",
    );
    checked.wants(
        "the copy names nobody after a give-up",
        Layout::open(&copy)
            .expect("the fetched copy")
            .entry(ENTRY)
            .and_then(|data| data.owner().map(str::to_owned))
            .is_none(),
        "the copy still named a holder",
    );

    // An entry the Vault does not hold is told apart from one it holds and nobody owns, and from
    // one the copy does not name.
    let said = run(&mut client(
        &workspace,
        &data,
        &["layout", "req-ownership", VAULT_NAME, &gone],
    ));
    checked.wants(
        "taking an entry the Vault does not hold is refused",
        said.code == Some(192) && clean(&said.stderr).contains("holds no entry"),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    let said = run(&mut client(
        &workspace,
        &data,
        &["layout", "read-ownership", VAULT_NAME, &gone],
    ));
    checked.wants(
        "reading an entry the copy does not name is refused",
        said.code == Some(192) && clean(&said.stderr).contains("names no entry"),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    // Locating and deprecating are a move of the path in the Vault's Layout, and it is the
    // holder's to make: take the entry again, then move it under the deprecated marker.
    run(&mut client(
        &workspace,
        &data,
        &["layout", "req-ownership", VAULT_NAME, &entry],
    ))
    .expect_success();

    let said = run(&mut client(
        &workspace,
        &data,
        &[
            "layout",
            "path",
            "move",
            "a.psd",
            "#/removed/a.psd",
            "--layout",
            "truth@origin",
        ],
    ));
    checked.wants(
        "a holder moves a path in the Vault's Layout",
        said.success() && vault_path(&root).as_deref() == Some("#/removed/a.psd"),
        &format!(
            "it ended with {:?} and said {:?}",
            said.code,
            said.stdout.trim()
        ),
    );
    checked.wants(
        "the copy follows the path the Vault moved",
        Layout::open(&copy)
            .expect("the fetched copy")
            .id_of(&LayoutPath::new("#/removed/a.psd").unwrap())
            == Some(ENTRY),
        "the copy did not follow the move",
    );

    // And the same entry now reads as deprecated, which is what the marker is for.
    let said = run(&mut client(
        &workspace,
        &data,
        &[
            "layout",
            "entries",
            "--layout",
            "truth@origin",
            "--uuid",
            &entry,
            "--json",
        ],
    ));
    checked.wants(
        "an entry the Vault deprecated says so",
        said.success() && said.stdout.contains("\"deprecated\":true"),
        &format!(
            "it ended with {:?} and said {:?}",
            said.code,
            said.stdout.trim()
        ),
    );

    // A member who neither holds the entry nor administrates the Vault may not move it.
    run(&mut client(&workspace, &data, &["account", BOB])).expect_success();
    let said = run(&mut client(
        &workspace,
        &data,
        &[
            "layout",
            "path",
            "move",
            "#/removed/a.psd",
            "a.psd",
            "--layout",
            "truth@origin",
        ],
    ));
    checked.wants(
        "a member who does not hold the entry may not move it",
        said.code == Some(193) && clean(&said.stderr).contains("Only the entry's holder"),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    // A move that cannot be made says which way, and `#` is reserved for the two markers.
    run(&mut client(&workspace, &data, &["account", ALICE])).expect_success();

    let said = run(&mut client(
        &workspace,
        &data,
        &[
            "layout",
            "path",
            "move",
            "nowhere.psd",
            "Models/nowhere.psd",
            "--layout",
            "truth@origin",
        ],
    ));
    checked.wants(
        "moving a path the Vault does not name is not found",
        said.code == Some(11),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    let said = run(&mut client(
        &workspace,
        &data,
        &[
            "layout",
            "path",
            "move",
            "#/removed/a.psd",
            "#/archive/a.psd",
            "--layout",
            "truth@origin",
        ],
    ));
    checked.wants(
        "a marker the design does not name is refused",
        said.code == Some(191),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    let said = run(&mut client(
        &workspace,
        &data,
        &[
            "layout",
            "path",
            "move",
            "#/removed/a.psd",
            "#/removed/a.psd",
            "--layout",
            "truth@origin",
        ],
    ));
    checked.wants(
        "moving a path onto itself is already taken",
        said.code == Some(10),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    // A sync is about the Layout being worked in and the Vault it tracks: name the Vault, then a
    // `--dry-run` makes the plan from what the Vault holds and changes nothing.
    run(&mut client(
        &workspace,
        &data,
        &["layout", "set-track", "main", VAULT_NAME],
    ))
    .expect_success();

    let said = run(&mut client(
        &workspace,
        &data,
        &["sync", "--dry-run", "--json"],
    ));
    checked.wants(
        "a dry run plans one entry from the Vault alone",
        said.success()
            && said.stdout.contains("\"layout\":\"main\"")
            && said.stdout.contains("\"vault\":\"origin\"")
            && said.stdout.contains("\"remote_only\"")
            && said.stdout.contains("\"deprecated\":true"),
        &format!(
            "it ended with {:?} and said {:?}",
            said.code,
            said.stdout.trim()
        ),
    );

    // A file tracked here and not in the Vault goes up under `#/new/`, named by its path and the
    // short form of its `Uuid`.
    let fresh: Uuid = Uuid::from_u128(0xaa);
    let version = "33".repeat(32);
    run(&mut client(
        &workspace,
        &data,
        &[
            "layout",
            "entry",
            "create",
            &fresh.to_string(),
            &version,
            "--owner",
            ALICE,
        ],
    ))
    .expect_success();
    run(&mut client(
        &workspace,
        &data,
        &["layout", "path", "create", "art/hero.psd", &fresh.to_string()],
    ))
    .expect_success();

    let said = run(&mut client(&workspace, &data, &["sync", "--up-only"]));
    checked.wants(
        "a file only this Layout holds goes up under #/new/",
        said.success()
            && vault_path_of(&root, fresh)
                == Some("#/new/art/hero.psd@0000000".to_owned()),
        &format!(
            "it ended with {:?}, said {:?}, and the Vault names {:?}",
            said.code,
            said.stdout.trim(),
            vault_path_of(&root, fresh)
        ),
    );
    checked.wants(
        "the Vault holds what went up",
        vault_owner_of(&root, fresh).as_deref() == Some(ALICE),
        &format!("the Vault holds {:?}", vault_owner_of(&root, fresh)),
    );

    // What the Vault took was written into the fetched copy as it was taken, so a reading command
    // sees the Vault as the run left it rather than as it found it — no fetch of its own needed.
    let said = run(&mut client(
        &workspace,
        &data,
        &[
            "layout",
            "entries",
            "--layout",
            "truth@origin",
            "--uuid",
            &fresh.to_string(),
            "--format",
            "{{ entries.path }}",
        ],
    ));
    checked.wants(
        "the fetched copy follows what went up",
        said.success() && said.stdout.contains("#/new/art/hero.psd@0000000"),
        &format!(
            "it ended with {:?} and said {:?}",
            said.code,
            said.stdout.trim()
        ),
    );

    // A tracked file has a version, not just a name: sending it puts that version in the Vault,
    // and a version the Vault is ahead on comes back down into the tree.
    //
    // The Layout already names `art/hero.psd` from the layout-entry test above, and a name the
    // tree does not hold is what `track` refuses beside: give it something to be, since this run
    // is about the version below rather than that path.
    fs::create_dir_all(workspace.join("art")).expect("the art directory");
    fs::write(workspace.join("art/hero.psd"), b"a placeholder").expect("the placeholder");

    let model = workspace.join("models/hero.psd");
    fs::create_dir_all(model.parent().expect("somewhere to put it")).expect("the model directory");
    fs::write(&model, b"first").expect("the first model");

    run(&mut client(
        &workspace,
        &data,
        &["track", "models/hero.psd", "--message", "the first model"],
    ))
    .expect_success();

    let held = Layout::open(workspace.join(".rola/layouts/main")).expect("the Layout being worked in");
    let (id, first) = held
        .entries()
        .into_iter()
        .find(|(id, _)| {
            held.path_of(*id)
                .is_some_and(|path| path.as_str() == "models/hero.psd")
        })
        .expect("the tracked entry");
    let first = first.version();

    run(&mut client(&workspace, &data, &["sync", "--up-only"])).expect_success();

    fs::write(&model, b"second").expect("the second model");
    run(&mut client(
        &workspace,
        &data,
        &["track", "models/hero.psd", "--message", "the second model"],
    ))
    .expect_success();

    let held = Layout::open(workspace.join(".rola/layouts/main")).expect("the Layout being worked in");
    let second = held.entry(id).expect("the entry").version();

    run(&mut client(&workspace, &data, &["sync", "--up-only"])).expect_success();

    // Put this side back at the first version, with the tree matching it, so the Vault is ahead:
    // nothing here has been changed, and what the Vault holds is what a pull should bring.
    let held = Layout::open(workspace.join(".rola/layouts/main")).expect("the Layout being worked in");
    held.update_entry(
        id,
        MutableData::new(Some(ALICE.to_owned()), first, String::new()),
    )
    .expect("the first version");
    fs::write(&model, b"first").expect("the first model again");

    // The reading still remembers the second version as what the Layout agreed with, so the tree
    // is put back to the first version's content and recorded as agreeing with it.
    run(&mut client(
        &workspace,
        &data,
        &["align", "models/hero.psd", "--restore-modify"],
    ))
    .expect_success();

    let said = run(&mut client(
        &workspace,
        &data,
        &["sync", "--down-only", "--json"],
    ));
    checked.wants(
        "a version the Vault is ahead on comes down into the tree",
        said.success() && said.stdout.contains("\"received\":1") && fs::read(&model).unwrap() == b"second",
        &format!(
            "it ended with {:?}, said {:?}, and the tree holds {:?}",
            said.code,
            said.stdout.trim(),
            fs::read(&model)
        ),
    );
    checked.wants(
        "the Layout follows the version that came down",
        Layout::open(workspace.join(".rola/layouts/main"))
            .expect("the Layout being worked in")
            .entry(id)
            .expect("the entry")
            .version()
            == second,
        "the Layout did not follow the version down",
    );

    // A `Uuid` the Vault holds but this Layout does not is brought in by naming where it goes. In
    // the Layout that already holds it, that is refused; in a Workspace that never did, it is how
    // the file arrives at all — content and all.
    let plain = id.simple().to_string();
    let remote_path = format!("#/new/models/hero.psd@{}", &plain[..7]);

    let said = run(&mut client(
        &workspace,
        &data,
        &["checkin", &remote_path, "--to-local", "elsewhere/hero.psd"],
    ));
    checked.wants(
        "checking in what this Layout already holds is refused",
        said.code == Some(240),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    let said = run(&mut client(
        &workspace,
        &data,
        &[
            "checkin",
            &remote_path,
            "a.psd",
            "--to-local",
            "only/one.psd",
        ],
    ));
    checked.wants(
        "references and paths that do not line up are refused",
        said.code == Some(241),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    let other = sandbox.join("ws2");
    Workspace::create(&other).expect("a second workspace");

    // The same account, so the Vault admits the run: the keys belong to the Workspace, so they are
    // copied beside the new one rather than made again — a new pair would be a stranger.
    let auth = other.join(".rola/auth");
    fs::create_dir_all(&auth).expect("the second workspace's keys");
    for suffix in ["pem", "pub"] {
        fs::copy(
            workspace.join(format!(".rola/auth/{ALICE}.{suffix}")),
            auth.join(format!("{ALICE}.{suffix}")),
        )
        .expect("the account's key");
    }

    run(&mut client(&other, &data, &["account", ALICE])).expect_success();
    run(&mut client(
        &other,
        &data,
        &["vault", "bind", VAULT_NAME, &address(PORT)],
    ))
    .expect_success();
    run(&mut client(
        &other,
        &data,
        &["layout", "set-track", "main", VAULT_NAME],
    ))
    .expect_success();

    let said = run(&mut client(
        &other,
        &data,
        &[
            "checkin",
            &remote_path,
            "--to-local",
            "copied/hero.psd",
            "--json",
        ],
    ));
    checked.wants(
        "a checkin brings what only the Vault holds into a fresh Workspace",
        said.success()
            && fs::read(other.join("copied/hero.psd")).unwrap() == b"second"
            && said.stdout.contains(&id.to_string()),
        &format!(
            "it ended with {:?}, said {:?}, and the tree holds {:?}",
            said.code,
            said.stdout.trim(),
            fs::read(other.join("copied/hero.psd"))
        ),
    );
    checked.wants(
        "the Layout names what was checked in",
        Layout::open(other.join(".rola/layouts/main"))
            .expect("the second Layout")
            .id_of(&LayoutPath::new("copied/hero.psd").unwrap())
            == Some(id),
        "the second Layout does not name what came in",
    );

    // A Layout that tracks no Vault has nothing to sync with.
    run(&mut client(
        &workspace,
        &data,
        &["layout", "new", "untracked"],
    ))
    .expect_success();
    run(&mut client(
        &workspace,
        &data,
        &["layout", "force-switch", "untracked"],
    ))
    .expect_success();

    let said = run(&mut client(&workspace, &data, &["sync", "--dry-run"]));
    checked.wants(
        "a Layout that tracks no Vault has nothing to sync with",
        said.code == Some(230),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    run(&mut client(
        &workspace,
        &data,
        &["layout", "force-switch", "main"],
    ))
    .expect_success();

    // Nothing is left to send, so a plain run moves only content, and two directions at once is
    // nobody's.
    let said = run(&mut client(&workspace, &data, &["sync", "--json"]));
    checked.wants(
        "a run with nothing to send still moves content",
        said.success()
            && said.stdout.contains("\"applied\":0")
            && said.stdout.contains("\"failed\":[]"),
        &format!(
            "it ended with {:?} and said {:?}",
            said.code,
            said.stdout.trim()
        ),
    );

    // A run says what it is doing while it does it: the fetch and the plan are one bar of two
    // steps, told as records since this run is read by a program rather than watched.
    checked.wants(
        "a run says what it is doing while it does it",
        said.stderr.contains("\"signal\":\"begin\"") && said.stderr.contains("\"total\":2"),
        &format!("it said {:?}", said.stderr.trim()),
    );

    let said = run(&mut client(
        &workspace,
        &data,
        &["sync", "--up-only", "--down-only", "--dry-run"],
    ));
    checked.wants(
        "two directions at once are refused",
        said.code == Some(231),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    // A file the fetched copy says another account holds is not this account's to change: what the
    // tree did to it is said first, and recording it is refused unless the run means it.
    Layout::open(root.join(LAYOUT_DIR))
        .expect("the Vault's Layout")
        .update_entry(
            id,
            MutableData::new(Some(BOB.to_owned()), second, String::new()),
        )
        .expect("another holder");
    run(&mut client(
        &workspace,
        &data,
        &["layout", "fetch", VAULT_NAME],
    ))
    .expect_success();

    fs::write(&model, b"third").expect("a change of my own");

    let said = run(&mut client(&workspace, &data, &["status"]));
    checked.wants(
        "a change to a file another account holds is said first",
        said.success()
            && clean(&said.stdout).contains("not yours to make")
            && clean(&said.stdout).contains("models/hero.psd"),
        &format!(
            "it ended with {:?} and said {:?}",
            said.code,
            clean(&said.stdout)
        ),
    );

    let said = run(&mut client(
        &workspace,
        &data,
        &["track", "models/hero.psd", "--message", "mine now"],
    ));
    checked.wants(
        "recording a file another account holds is refused",
        said.code == Some(202),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    let said = run(&mut client(
        &workspace,
        &data,
        &[
            "track",
            "models/hero.psd",
            "--message",
            "mine now",
            "--force",
        ],
    ));
    checked.wants(
        "a run that means it records the file anyway",
        said.success(),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    // `hold` and `giveup` are the same ownership the two `layout` commands move, with the gates
    // that make either safe: the Vault is fetched first, what is here has to be what the Vault
    // names and settled, and who holds it has to be who the command expects.
    Layout::open(root.join(LAYOUT_DIR))
        .expect("the Vault's Layout")
        .update_entry(ENTRY, MutableData::new(None, [7; 32], "a file".to_owned()))
        .expect("nobody holds it");
    run(&mut client(
        &workspace,
        &data,
        &["layout", "fetch", VAULT_NAME],
    ))
    .expect_success();

    let held_path = "held.psd";
    let held_version = format!("blake3:{}", "07".repeat(32));
    run(&mut client(
        &workspace,
        &data,
        &[
            "layout",
            "entry",
            "create",
            &entry,
            &held_version,
            "--owner",
            ALICE,
        ],
    ))
    .expect_success();
    run(&mut client(
        &workspace,
        &data,
        &["layout", "path", "create", held_path, &entry],
    ))
    .expect_success();
    fs::write(workspace.join(held_path), b"held").expect("the held file");

    let said = run(&mut client(&workspace, &data, &["hold", held_path]));
    checked.wants(
        "a file nobody holds is claimed",
        said.success() && vault_owner_of(&root, ENTRY).as_deref() == Some(ALICE),
        &format!(
            "it ended with {:?}, said {:?}, and the Vault holds {:?}",
            said.code,
            said.stderr.trim(),
            vault_owner_of(&root, ENTRY)
        ),
    );

    let said = run(&mut client(&workspace, &data, &["hold", held_path]));
    checked.wants(
        "claiming what is already held is refused",
        said.code == Some(193),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    let said = run(&mut client(&workspace, &data, &["giveup", held_path]));
    checked.wants(
        "a file this account holds is let go of",
        said.success() && vault_owner_of(&root, ENTRY).is_none(),
        &format!(
            "it ended with {:?}, said {:?}, and the Vault holds {:?}",
            said.code,
            said.stderr.trim(),
            vault_owner_of(&root, ENTRY)
        ),
    );

    let said = run(&mut client(&workspace, &data, &["giveup", held_path]));
    checked.wants(
        "letting go of what nobody holds is refused",
        said.code == Some(193),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    // What is here has to be the version the Vault names, and `--force` is what moves past it.
    Layout::open(root.join(LAYOUT_DIR))
        .expect("the Vault's Layout")
        .update_entry(ENTRY, MutableData::new(None, [8; 32], "a file".to_owned()))
        .expect("another version");
    run(&mut client(
        &workspace,
        &data,
        &["layout", "fetch", VAULT_NAME],
    ))
    .expect_success();

    let said = run(&mut client(&workspace, &data, &["hold", held_path]));
    checked.wants(
        "claiming a version the Vault has moved past is refused",
        said.code == Some(193),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    let said = run(&mut client(&workspace, &data, &["hold", held_path, "--force"]));
    checked.wants(
        "a claim that means it moves past the version",
        said.success() && vault_owner_of(&root, ENTRY).as_deref() == Some(ALICE),
        &format!(
            "it ended with {:?}, said {:?}, and the Vault holds {:?}",
            said.code,
            said.stderr.trim(),
            vault_owner_of(&root, ENTRY)
        ),
    );

    serving.stop();
    checked.report();
}

/// The text of `said` with any terminal escape sequences taken out.
///
/// Output that has been piped is not read by a terminal, so what it holds is the line itself — but a
/// check that compares against it should not depend on that. This takes the escapes out either way.
fn clean(said: &str) -> String {
    let mut clean = String::with_capacity(said.len());
    let mut rest = said;

    while let Some(start) = rest.find('\u{1b}') {
        clean.push_str(&rest[..start]);

        // An escape sequence ends at `m`; anything after it is what follows.
        match rest[start..].find('m') {
            Some(end) => rest = &rest[start + end + 1..],
            None => return clean,
        }
    }
    clean.push_str(rest);

    clean
}

/// A command that runs the client, working in `workspace` and keeping its own files under `data`.
fn client(workspace: &Path, data: &Path, args: &[&str]) -> Command {
    let mut command = command("rola");

    command
        .current_dir(workspace)
        .env("XDG_DATA_HOME", data)
        // What the run says is checked, so it is said in one language rather than whichever one
        // the machine happens to be set to.
        .env("ROLA_LANG", "en")
        .args(args);

    command
}

/// Who holds [`ENTRY`] in the Vault's own Layout, read afresh.
///
/// The Vault is another process, so its Layout is opened again rather than kept: what this process
/// read before is what was true then, and only a fresh read sees what the Vault's daemon wrote.
fn vault_owner(root: &Path) -> Option<String> {
    vault_owner_of(root, ENTRY)
}

/// Who holds `id` in the Vault's own Layout, read afresh.
fn vault_owner_of(root: &Path, id: Uuid) -> Option<String> {
    Layout::open(root.join(LAYOUT_DIR))
        .expect("the Vault's Layout")
        .entry(id)
        .and_then(|data| data.owner().map(str::to_owned))
}

/// The path [`ENTRY`] is at in the Vault's own Layout, read afresh.
fn vault_path(root: &Path) -> Option<String> {
    vault_path_of(root, ENTRY)
}

/// The path `id` is at in the Vault's own Layout, read afresh.
fn vault_path_of(root: &Path, id: Uuid) -> Option<String> {
    Layout::open(root.join(LAYOUT_DIR))
        .expect("the Vault's Layout")
        .path_of(id)
        .map(|path| path.as_str().to_owned())
}

/// The address a Vault serving on `port` is reached at.
fn address(port: u16) -> String {
    format!("127.0.0.1:{port}")
}

/// Makes `dir` a Vault that can be served: a Vault of its own, listening on `port`, holding the
/// account it proves itself with.
fn serveable(dir: &Path, port: u16) {
    Vault::create(dir).unwrap_or_else(|error| panic!("making {}: {error:?}", dir.display()));

    fs::write(
        dir.join(CONFIG_PATH),
        format!("[daemon_config]\nprefer_port = {port}\n"),
    )
    .expect("writing the Vault's configuration");

    pair(&dir.join(KEYS_DIR), "vault");
}

/// Puts the public half of the pair `name` names, kept under `keys`, where `vault` admits it.
fn publish(keys: &Path, vault: &Path, name: &str) {
    let public = keys.join(format!("{name}.pub"));
    let admitted = vault.join(KEYS_DIR);

    fs::create_dir_all(&admitted).expect("the Vault's keys directory");
    fs::copy(&public, admitted.join(format!("{name}.pub")))
        .unwrap_or_else(|error| panic!("publishing {}: {error}", public.display()));
}

/// Writes the key pair `name` names under `dir`.
fn pair(dir: &Path, name: &str) {
    fs::create_dir_all(dir).expect("the keys directory");
    let private = dir.join(format!("{name}.pem"));
    let public = dir.join(format!("{name}.pub"));

    run(Command::new("openssl")
        .args(["genpkey", "-algorithm", "ed25519", "-out"])
        .arg(&private))
    .expect_success();

    run(Command::new("openssl")
        .args(["pkey", "-in"])
        .arg(&private)
        .args(["-pubout", "-out"])
        .arg(&public))
    .expect_success();
}

/// Serves the Vault rooted at `dir`, and waits until it is listening on `port`.
fn serve_vault(dir: &Path, port: u16) -> Serving {
    serve(
        command("rola-daemon")
            .arg("--vault-dir")
            .arg(dir)
            .arg("listen"),
        ("127.0.0.1", port),
        STARTUP,
    )
}

/// What was asked of the commands, and how many of the questions were answered as they should be.
#[derive(Default)]
struct Checked {
    /// How many questions were asked.
    asked: usize,
    /// Which of them were answered with something other than what was wanted.
    wrong: Vec<String>,
}

impl Checked {
    /// Counts a question, and records it as wrong when it was not answered as it should be.
    fn wants(&mut self, question: &str, answered: bool, said: &str) {
        self.asked += 1;

        if !answered {
            self.wrong.push(format!("{question} — {said}"));
        }
    }

    /// Says what was asked and what was wrong with the answers, and ends the program on them.
    fn report(self) {
        for said in &self.wrong {
            eprintln!("FAIL: {said}");
        }

        if self.wrong.is_empty() {
            println!(
                "{} questions, every one answered as it should be",
                self.asked
            );
            return;
        }

        eprintln!(
            "{} of {} answered as they should not",
            self.wrong.len(),
            self.asked
        );
        std::process::exit(1);
    }
}

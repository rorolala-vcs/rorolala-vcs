//! `fetch-with`: bringing the content of the entries a Layout names.
//!
//! What a fetch brings is a Vault's Layout, and a Layout only names versions. This is the check on
//! the fetch that brings what those versions stored: a first Workspace makes two versions of a
//! file, a directory of two files and a tree, uploads them, and a second Workspace, holding none of
//! it, asks for one entry at a time and is checked on what arrived.
//!
//! What is checked: that a shallow fetch brings the version an entry is at and not the one below
//! it, that `--recursive` brings the whole chain, that a tree comes with what its structure names,
//! that a directory word takes every entry under it, that a word naming nothing is passed over,
//! that `--recursive` beside `--shallow` and two Vaults named at once are refused, and that a Vault
//! whose index or store cannot answer is told apart by exit code.
//!
//! A program rather than a `#[test]` for the same reason the other suites are: it runs the built
//! programs the way a caller meets them, over a real socket, with real daemons serving real Vaults.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use librorolala::layout::{Layout, LayoutPath, MutableData};
use librorolala::storage::Key;
use librorolala::tree::TreeStructure;
use librorolala::vault::{INDEX_DIR, KEYS_DIR, LAYOUT_DIR, Vault};
use librorolala::vcs::{ComputeBlake3 as _, Creator, Message, VCSIndex, VCSIndexObject, Version};
use librorolala::workspace::{Workspace, locate_workspace};
use rorolala_utils_sandbox::{Guard, Serving, command, run, serve};
use uuid::Uuid;

/// The port the Vault holding the work serves on.
const PORT: u16 = 7945;

/// The port the Vault holding a version whose content is nowhere serves on.
const MIRROR_PORT: u16 = 7946;

/// The namespaces the two Vaults are registered under.
const ORIGIN: &str = "origin";
const MIRROR: &str = "mirror";

/// The account both Workspaces act as, which is a member of both Vaults.
const NAME: &str = "rootuser";

/// How long a Vault is waited for before it is given up on.
const STARTUP: Duration = Duration::from_secs(10);

/// The entry of the mirror Vault whose version the index holds and whose content is nowhere.
const HELD: Uuid = Uuid::from_u128(0x91);

/// The entry of the mirror Vault whose version the index does not hold.
const UNHELD: Uuid = Uuid::from_u128(0x92);

/// The exit code a refusal of the flags ends with.
const ARGUMENT: i32 = 191;

/// The exit code a run whose content did not all arrive ends with.
const INCOMPLETE: i32 = 243;

/// The exit code a run whose index holds no chain ends with.
const NO_CHAIN: i32 = 242;

/// The exit code a hashed word that names nothing ends with.
const HASH_MISS: i32 = 245;

#[tokio::main]
async fn main() {
    let sandbox = Guard::new("fetch-with");
    let data = sandbox.join("data");
    let root = sandbox.join("origin");
    let mirror = sandbox.join("mirror");
    let first = sandbox.join("ws1");
    let second = sandbox.join("ws2");

    let mut checked = Checked::default();

    // Two Vaults, one of which holds everything the work is made of, and one of which holds a
    // Layout and an index that name versions whose content is not there.
    Vault::create(&root).unwrap_or_else(|error| panic!("making {}: {error:?}", root.display()));
    Vault::create(&mirror).unwrap_or_else(|error| panic!("making {}: {error:?}", mirror.display()));
    Workspace::create(&first)
        .unwrap_or_else(|error| panic!("making {}: {error:?}", first.display()));
    Workspace::create(&second)
        .unwrap_or_else(|error| panic!("making {}: {error:?}", second.display()));

    let auth = first.join(".rola").join("auth");
    pair(&auth, NAME);
    publish(&auth, &root, NAME);
    publish(&auth, &mirror, NAME);

    run(&mut client(&first, &data, &["account", NAME])).expect_success();
    run(&mut client(&first, &data, &["bind", &link(ORIGIN, PORT)])).expect_success();

    // A file with two versions, a directory of two files, and a tree of two files: the three shapes
    // a fetch of content has to read.
    fs::write(first.join("hero.psd"), b"hero one").expect("the first save");
    run(&mut client(
        &first,
        &data,
        &["track", "hero.psd", "--message", "1"],
    ))
    .expect_success();
    fs::write(first.join("hero.psd"), b"hero two").expect("the second save");
    run(&mut client(
        &first,
        &data,
        &["track", "hero.psd", "--message", "2"],
    ))
    .expect_success();

    fs::create_dir_all(first.join("plain")).expect("a directory to record");
    fs::write(first.join("plain").join("one.psd"), b"one").expect("one file");
    fs::write(first.join("plain").join("two.psd"), b"two").expect("another file");
    run(&mut client(
        &first,
        &data,
        &["track", "plain", "--message", "plain"],
    ))
    .expect_success();

    fs::create_dir_all(first.join("main")).expect("a directory to make a tree of");
    fs::write(first.join("main").join("a.psd"), b"tree a").expect("the first tree file");
    fs::write(first.join("main").join("b.psd"), b"tree b").expect("the second tree file");
    run(&mut client(&first, &data, &["tree", "init", "main"])).expect_success();
    run(&mut client(&first, &data, &["tree", "add", "main"])).expect_success();
    run(&mut client(
        &first,
        &data,
        &["track", "main", "--message", "tree"],
    ))
    .expect_success();

    // A file and a tree that only the hash words reach, so what a version below the top and a
    // structure's files come to is not answered by a path fetch made earlier.
    fs::write(first.join("hashed.psd"), b"hashed one").expect("the first hashed save");
    run(&mut client(
        &first,
        &data,
        &["track", "hashed.psd", "--message", "1"],
    ))
    .expect_success();
    fs::write(first.join("hashed.psd"), b"hashed two").expect("the second hashed save");
    run(&mut client(
        &first,
        &data,
        &["track", "hashed.psd", "--message", "2"],
    ))
    .expect_success();

    fs::create_dir_all(first.join("hashed-tree")).expect("a directory to make a tree of");
    fs::write(first.join("hashed-tree").join("x.psd"), b"hash tree x").expect("a tree file");
    fs::write(first.join("hashed-tree").join("y.psd"), b"hash tree y").expect("another tree file");
    run(&mut client(&first, &data, &["tree", "init", "hashed-tree"])).expect_success();
    run(&mut client(&first, &data, &["tree", "add", "hashed-tree"])).expect_success();
    run(&mut client(
        &first,
        &data,
        &["track", "hashed-tree", "--message", "tree"],
    ))
    .expect_success();

    mirror_versions(&mirror).await;

    register(&data, ORIGIN, &root);
    register(&data, MIRROR, &mirror);
    let origin = serve_vault(&data, PORT);
    let nowhere = serve_vault(&data, MIRROR_PORT);

    run(&mut client(&first, &data, &["sync"])).expect_success();

    // What the Vault's own Layout, index and store say each entry is made of, which is what the
    // fetch is checked against: the answer comes from the Vault rather than from the run.
    let vault_layout = Layout::open(root.join(LAYOUT_DIR)).expect("the Vault's Layout");
    let vault_index = VCSIndex::create(root.join(INDEX_DIR));
    let vault_store = Vault::at(root.as_path())
        .get_current_rola_storage()
        .expect("the Vault's store");

    let (_, hero_version) = entry(&vault_layout, "hero.psd");
    let hero_chain = chain(&vault_index, hero_version).await;
    let (_, one_version) = entry(&vault_layout, "plain/one.psd");
    let (_, two_version) = entry(&vault_layout, "plain/two.psd");
    let plain_keys = [
        storage_of(&vault_index, one_version).await,
        storage_of(&vault_index, two_version).await,
    ];

    let (tree_uuid, tree_version) = entry(&vault_layout, "main");
    let tree_key = storage_of(&vault_index, tree_version).await;
    let structure = vault_store
        .read_object(&tree_key)
        .await
        .map(|bytes| TreeStructure::decode(&bytes).expect("a tree structure"))
        .expect("the tree's structure");
    let tree_contents = structure.content_keys();

    // The hashed file's chain, its variants, and the tree only the hash words reach.
    let (_, hashed_version) = entry(&vault_layout, "hashed.psd");
    let hashed_versions = versions(&vault_index, hashed_version).await;
    assert_eq!(
        hashed_versions.len(),
        2,
        "the hashed file is recorded twice, which is a chain of two"
    );
    let hashed_head = storage_of(&vault_index, hashed_versions[0]).await;
    let hashed_older = storage_of(&vault_index, hashed_versions[1]).await;
    let hashed_head_variant = variant_of(&vault_index, hashed_versions[0]).await;
    let hashed_older_variant = variant_of(&vault_index, hashed_versions[1]).await;
    let hashed_older_short = hashed_older_variant.hex()[..7].to_owned();

    let (_, hashed_tree_version) = entry(&vault_layout, "hashed-tree");
    let hashed_tree_variant = variant_of(&vault_index, hashed_tree_version).await;
    let hashed_tree_key = storage_of(&vault_index, hashed_tree_version).await;
    let hashed_structure = vault_store
        .read_object(&hashed_tree_key)
        .await
        .map(|bytes| TreeStructure::decode(&bytes).expect("a tree structure"))
        .expect("the hashed tree's structure");
    let hashed_tree_contents = hashed_structure.content_keys();

    // The second Workspace acts as the same member, so the Vault admits it, and reaches both Vaults
    // by name.
    copy_identity(&auth, &second.join(".rola").join("auth"));
    run(&mut client(&second, &data, &["account", NAME])).expect_success();
    run(&mut client(&second, &data, &["bind", &link(ORIGIN, PORT)])).expect_success();
    run(&mut client(
        &second,
        &data,
        &[
            "bind",
            &link(MIRROR, MIRROR_PORT),
            MIRROR,
            "--no-set-default-vault",
        ],
    ))
    .expect_success();

    // A hash names an object of the index: a variant names the content it points at, and a version
    // the variant it fixes. The local check says whether that content is here, before and after.
    let said = run(&mut client(
        &second,
        &data,
        &[
            "vcs-index",
            "can-retrack-offline",
            &hashed_older_variant.hex(),
        ],
    ));
    checked.wants(
        "a variant's content is not here before it is fetched",
        said.success() && said.stdout.trim().ends_with("no"),
        &format!(
            "it ended with {:?} and said {:?}",
            said.code,
            said.stdout.trim()
        ),
    );

    let older_version = Key::new(hashed_versions[1]).hex();
    let said = run(&mut client(
        &second,
        &data,
        &["vcs-index", "can-retrack-offline", &older_version],
    ));
    checked.wants(
        "a version's content is not here before it is fetched",
        said.success() && said.stdout.trim().ends_with("no"),
        &format!(
            "it ended with {:?} and said {:?}",
            said.code,
            said.stdout.trim()
        ),
    );

    let said = run(&mut client(
        &second,
        &data,
        &["fetch", "--with", &hashed_older_variant.hex(), "--shallow"],
    ));
    checked.wants(
        "a fetch of a variant's hash succeeds",
        said.success(),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    let presence = held(&second, &[hashed_older, hashed_head]).await;
    checked.wants(
        "the content the variant points at is here",
        presence[0],
        "the variant's content is not in the second Workspace's store",
    );
    checked.wants(
        "a shallow fetch of a variant brings nothing above it",
        !presence[1],
        "a shallow fetch of a variant brought the version above it",
    );

    let said = run(&mut client(
        &second,
        &data,
        &["vcs-index", "can-retrack-offline", &older_version],
    ));
    checked.wants(
        "a version is usable offline once its content is here",
        said.success() && said.stdout.trim().ends_with("yes"),
        &format!(
            "it ended with {:?} and said {:?}",
            said.code,
            said.stdout.trim()
        ),
    );

    // The whole chain is what a recursive fetch from the head variant carries: the variant's own
    // content, then the version it points at and every version that one stands on.
    let said = run(&mut client(
        &second,
        &data,
        &["fetch", "--with", &hashed_head_variant.hex(), "--recursive"],
    ));
    checked.wants(
        "a recursive fetch of a variant succeeds",
        said.success(),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    let presence = held(&second, &[hashed_head, hashed_older]).await;
    checked.wants(
        "a recursive fetch brings the chain behind the version it points at",
        presence.iter().all(|held| *held),
        "a version of the chain is missing from the second Workspace's store",
    );

    // A version's hash names the variant it fixes, so the same content is reachable by either name.
    let head_version = Key::new(hashed_versions[0]).hex();
    let said = run(&mut client(
        &second,
        &data,
        &["fetch", "--with", &head_version, "--shallow"],
    ));
    checked.wants(
        "a fetch of a version's hash succeeds",
        said.success() && said.stdout.contains("Fetched the content of 1 entries"),
        &format!(
            "it ended with {:?} and said {:?}",
            said.code,
            said.stdout.trim()
        ),
    );

    // A tree's variant is a structure, and the files it names are the second ask.
    let said = run(&mut client(
        &second,
        &data,
        &[
            "vcs-index",
            "can-retrack-offline",
            &hashed_tree_variant.hex(),
        ],
    ));
    checked.wants(
        "a tree's content is not here before it is fetched",
        said.success() && said.stdout.trim().ends_with("no"),
        &format!(
            "it ended with {:?} and said {:?}",
            said.code,
            said.stdout.trim()
        ),
    );

    let said = run(&mut client(
        &second,
        &data,
        &["fetch", "--with", &hashed_tree_variant.hex(), "--shallow"],
    ));
    checked.wants(
        "a fetch of a tree's variant succeeds",
        said.success(),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    let mut hashed_tree_keys = vec![hashed_tree_key];
    hashed_tree_keys.extend(hashed_tree_contents.iter().copied());
    let presence = held(&second, &hashed_tree_keys).await;
    checked.wants(
        "the tree's structure and the files it names are here",
        presence.iter().all(|held| *held),
        "the tree's structure or one of its files is missing from the second Workspace's store",
    );

    let said = run(&mut client(
        &second,
        &data,
        &[
            "vcs-index",
            "can-retrack-offline",
            &hashed_tree_variant.hex(),
        ],
    ));
    checked.wants(
        "a tree is usable offline once its files are here",
        said.success() && said.stdout.trim().ends_with("yes"),
        &format!(
            "it ended with {:?} and said {:?}",
            said.code,
            said.stdout.trim()
        ),
    );

    // The head of a hash names the same object as the whole one.
    let said = run(&mut client(
        &second,
        &data,
        &["fetch", "--with", &hashed_older_short],
    ));
    checked.wants(
        "the head of a hash names its object",
        said.success() && said.stdout.contains("Fetched the content of 1 entries"),
        &format!(
            "it ended with {:?} and said {:?}",
            said.code,
            said.stdout.trim()
        ),
    );

    // A hash that names nothing is refused rather than passed over, which is the difference between
    // a hash and a path.
    let unknown = "ab".repeat(32);
    let said = run(&mut client(&second, &data, &["fetch", "--with", &unknown]));
    checked.wants(
        "a hash naming nothing is refused",
        said.code == Some(HASH_MISS),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    // A shallow fetch brings the version the entry is at, and not the one below it.
    let said = run(&mut client(
        &second,
        &data,
        &[
            "fetch",
            "--with",
            "hero.psd",
            "--layout",
            "truth@origin",
            "--shallow",
        ],
    ));
    checked.wants(
        "a shallow fetch of one file succeeds",
        said.success(),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );
    checked.wants(
        "a shallow fetch says how many entries came",
        said.stdout.contains("Fetched the content of 1 entries"),
        &format!("it said {:?}", said.stdout.trim()),
    );

    let presence = held(&second, &hero_chain).await;
    checked.wants(
        "the version the entry is at is here",
        presence.first().copied().unwrap_or(false),
        "the head version's content is not in the second Workspace's store",
    );
    checked.wants(
        "the version below it is not here",
        presence.get(1).is_some_and(|held| !held),
        "the version below the head was brought by a shallow fetch",
    );

    // A recursive fetch follows the version back to its root.
    let said = run(&mut client(
        &second,
        &data,
        &[
            "fetch",
            "--with",
            "hero.psd",
            "--layout",
            "truth@origin",
            "--recursive",
        ],
    ));
    checked.wants(
        "a recursive fetch of one file succeeds",
        said.success(),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    let presence = held(&second, &hero_chain).await;
    checked.wants(
        "the whole chain behind the version is here",
        presence.iter().all(|held| *held),
        "a version of the chain is missing from the second Workspace's store",
    );

    // A tree's version is a structure, and what the structure names is a second ask.
    let said = run(&mut client(
        &second,
        &data,
        &[
            "fetch",
            "--with",
            "main",
            "--layout",
            "truth@origin",
            "--shallow",
        ],
    ));
    checked.wants(
        "a shallow fetch of a tree succeeds",
        said.success(),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    let mut tree_keys = vec![tree_key];
    tree_keys.extend(tree_contents.iter().copied());
    let presence = held(&second, &tree_keys).await;
    checked.wants(
        "the structure and what it names are here",
        presence.iter().all(|held| *held),
        "the tree's structure or one of its files is missing from the second Workspace's store",
    );

    // A directory word takes every entry under it.
    let said = run(&mut client(
        &second,
        &data,
        &[
            "fetch",
            "--with",
            "plain",
            "--layout",
            "truth@origin",
            "--shallow",
        ],
    ));
    checked.wants(
        "a fetch of a directory succeeds",
        said.success(),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );
    checked.wants(
        "a directory word covers every entry under it",
        said.stdout.contains("Fetched the content of 2 entries"),
        &format!("it said {:?}", said.stdout.trim()),
    );

    let presence = held(&second, &plain_keys).await;
    checked.wants(
        "every file of the directory is here",
        presence.iter().all(|held| *held),
        "a file of the directory is missing from the second Workspace's store",
    );

    // A word that names nothing is passed over, which is what a file never recorded is: the run is
    // the fetch of the Layout and nothing more.
    let said = run(&mut client(
        &second,
        &data,
        &[
            "fetch",
            "--with",
            "not-there.psd",
            "--layout",
            "truth@origin",
        ],
    ));
    checked.wants(
        "a word that names nothing leaves the fetch a fetch",
        said.success() && !said.stdout.contains("Fetched the content"),
        &format!(
            "it ended with {:?} and said {:?}",
            said.code,
            said.stdout.trim()
        ),
    );

    // A `Uuid` names its entry wherever the entry is at.
    let said = run(&mut client(
        &second,
        &data,
        &[
            "fetch",
            "--with",
            &tree_uuid.to_string(),
            "--layout",
            "truth@origin",
            "--shallow",
        ],
    ));
    checked.wants(
        "a Uuid names its entry",
        said.success() && said.stdout.contains("Fetched the content of 1 entries"),
        &format!(
            "it ended with {:?} and said {:?}",
            said.code,
            said.stdout.trim()
        ),
    );

    // The two flags say opposite things, and one run may not say both.
    let said = run(&mut client(
        &second,
        &data,
        &[
            "fetch",
            "--with",
            "hero.psd",
            "--layout",
            "truth@origin",
            "--recursive",
            "--shallow",
        ],
    ));
    checked.wants(
        "a run that names both depths is refused",
        said.code == Some(ARGUMENT),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    // A Layout of one Vault and a Vault named beside it are two answers to one question.
    let said = run(&mut client(
        &second,
        &data,
        &[
            "fetch",
            MIRROR,
            "--with",
            "hero.psd",
            "--layout",
            "truth@origin",
        ],
    ));
    checked.wants(
        "a Layout and a Vault that disagree are refused",
        said.code == Some(ARGUMENT),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    // A version the index names and the store does not hold is content that did not arrive, and the
    // entry it was asked for is what the run says.
    let said = run(&mut client(
        &second,
        &data,
        &["fetch", "--with", "held.psd", "--layout", "truth@mirror"],
    ));
    checked.wants(
        "content that is nowhere ends the run",
        said.code == Some(INCOMPLETE),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );
    checked.wants(
        "the entry whose content is missing is named",
        said.stderr.contains(&HELD.to_string()),
        &format!("it said {:?}", said.stderr.trim()),
    );

    // A version the index does not name is no chain at all, which is a different thing to say.
    let said = run(&mut client(
        &second,
        &data,
        &["fetch", "--with", "unheld.psd", "--layout", "truth@mirror"],
    ));
    checked.wants(
        "a version the index does not hold is no chain",
        said.code == Some(NO_CHAIN),
        &format!("it ended with {:?}: {}", said.code, said.stderr.trim()),
    );

    nowhere.stop();
    origin.stop();
    checked.report();
}

/// The version `path` names in `layout`, and the `Uuid` at it.
fn entry(layout: &Layout, path: &str) -> (Uuid, [u8; 32]) {
    let path = LayoutPath::new(path).expect("a path");
    let id = layout.id_of(&path).expect("an entry at the path");

    (id, layout.entry(id).expect("the entry's data").version())
}

/// The store key the version `version` names, as the index says.
async fn storage_of(index: &VCSIndex, version: [u8; 32]) -> Key {
    let object = index.read(Key::new(version)).await.expect("the version");
    let VCSIndexObject::Version(version) = object else {
        panic!("the index holds no version under the key the Layout names");
    };

    let object = index
        .read(Key::new(*version.variant()))
        .await
        .expect("the variant");
    let VCSIndexObject::Variant(variant) = object else {
        panic!("the index holds no variant under the version's name");
    };

    Key::new(*variant.storage_hash())
}

/// Every store key the chain from `head` back to its root names, head first.
async fn chain(index: &VCSIndex, head: [u8; 32]) -> Vec<Key> {
    let mut keys = Vec::new();
    let mut at = head;

    loop {
        let object = index.read(Key::new(at)).await.expect("the version");
        let VCSIndexObject::Version(version) = object else {
            break;
        };
        // The root is the version every chain ends at rather than one of its own, so it has no
        // variant and no content behind it.
        if version.is_root() {
            break;
        }

        let object = index
            .read(Key::new(*version.variant()))
            .await
            .expect("the variant");
        let VCSIndexObject::Variant(variant) = object else {
            break;
        };

        keys.push(Key::new(*variant.storage_hash()));
        at = *variant.base_version();
    }

    keys
}

/// The versions the chain from `head` back to its root names, head first.
async fn versions(index: &VCSIndex, head: [u8; 32]) -> Vec<[u8; 32]> {
    let mut chain = Vec::new();
    let mut at = head;

    loop {
        let object = index.read(Key::new(at)).await.expect("the version");
        let VCSIndexObject::Version(version) = object else {
            break;
        };
        // The root is the version every chain ends at rather than one of its own.
        if version.is_root() {
            break;
        }

        chain.push(at);

        let object = index
            .read(Key::new(*version.variant()))
            .await
            .expect("the variant");
        let VCSIndexObject::Variant(variant) = object else {
            break;
        };

        at = *variant.base_version();
    }

    chain
}

/// The variant a version fixes, as its key.
async fn variant_of(index: &VCSIndex, version: [u8; 32]) -> Key {
    let object = index.read(Key::new(version)).await.expect("the version");
    let VCSIndexObject::Version(version) = object else {
        panic!("the index holds no version under the key the Layout names");
    };

    Key::new(*version.variant())
}

/// Which of `keys` the store of `workspace` holds.
async fn held(workspace: &Path, keys: &[Key]) -> Vec<bool> {
    let store = locate_workspace(workspace)
        .expect("the Workspace the run was made in")
        .storage_set()
        .expect("the Workspace's store");
    let presence = store
        .contains_keys(keys)
        .await
        .expect("which keys are held");

    (0..keys.len()).map(|at| presence.held(at)).collect()
}

/// The mirror Vault's Layout and index: one version whose content is nowhere, and one version the
/// index does not hold.
async fn mirror_versions(root: &Path) {
    let creator = Creator::try_from(NAME).expect("a creator");
    let message = Message::try_from("a version whose content is nowhere").expect("a message");
    let variant = Version::root().new_variant(
        [0x77; 32],
        creator.compute_blake3(),
        message.compute_blake3(),
    );
    let version = variant.new_version();
    let version_hash = version.compute_blake3();

    let index = VCSIndex::create(root.join(INDEX_DIR));
    index.write(creator).await.expect("the creator");
    index.write(message).await.expect("the message");
    index.write(variant).await.expect("the variant");
    index.write(version).await.expect("the version");

    let layout = Layout::open(root.join(LAYOUT_DIR)).expect("the Vault's Layout");
    layout
        .create_entry(HELD, MutableData::new(None, version_hash, String::new()))
        .expect("an entry the index names");
    layout
        .create_path(&LayoutPath::new("held.psd").expect("a path"), HELD)
        .expect("the entry's path");
    layout
        .create_entry(UNHELD, MutableData::new(None, [0x99; 32], String::new()))
        .expect("an entry the index does not name");
    layout
        .create_path(&LayoutPath::new("unheld.psd").expect("a path"), UNHELD)
        .expect("the entry's path");
}

/// A command against `workspace`, acting as the machine's account and said in English.
fn client(workspace: &Path, data: &Path, args: &[&str]) -> Command {
    let mut command = command("rola");

    command
        .current_dir(workspace)
        .env("XDG_DATA_HOME", data)
        .env("ROLA_DATA_HOME", data)
        // What the run says is checked, so it is said in one language rather than whichever one
        // the machine happens to be set to.
        .env("ROLA_LANG", "en")
        .args(args);

    command
}

/// The link the Vault registered under `namespace` is reached by, on `port`.
fn link(namespace: &str, port: u16) -> String {
    format!("rola://127.0.0.1:{port}/{namespace}")
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

/// Copies the pair an account is, so a second Workspace acts as the same member.
fn copy_identity(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("the keys directory");
    for name in [format!("{NAME}.pem"), format!("{NAME}.pub")] {
        fs::copy(from.join(&name), to.join(&name))
            .unwrap_or_else(|error| panic!("copying {}: {error}", from.join(&name).display()));
    }
}

/// Puts the public half of the pair `name` names, kept under `keys`, where `vault` admits it.
fn publish(keys: &Path, vault: &Path, name: &str) {
    let public = keys.join(format!("{name}.pub"));
    let admitted = vault.join(KEYS_DIR);

    fs::create_dir_all(&admitted).expect("the Vault's keys directory");
    fs::copy(&public, admitted.join(format!("{name}.pub")))
        .unwrap_or_else(|error| panic!("publishing {}: {error}", public.display()));
}

/// Serves the Vault the namespace names, and waits until it is listening.
///
/// The daemon belongs to the machine, so it is told where it sits through the user's own data
/// directory: the sandbox's holds the table, which may name more than one Vault, and the account it
/// proves itself with.
fn serve_vault(data: &Path, port: u16) -> Serving {
    serve(
        command("rola-daemon")
            .arg("--port")
            .arg(port.to_string())
            .env("XDG_DATA_HOME", data)
            .env("ROLA_DATA_HOME", data),
        ("127.0.0.1", port),
        STARTUP,
    )
}

/// Adds `namespace` naming `dir` to the table the daemons read, and gives them an account of their
/// own.
fn register(data: &Path, namespace: &str, dir: &Path) {
    let registry = librorolala::location::user_data_dir_in(&data)
        .join("daemon")
        .join("registry.toml");

    fs::create_dir_all(registry.parent().expect("the daemon's directory"))
        .expect("the daemon's directory");

    let mut table = fs::read_to_string(&registry).unwrap_or_default();
    table.push_str(&format!("{namespace} = {dir:?}\n"));
    fs::write(&registry, table).expect("the namespace table");

    let keys = librorolala::location::user_data_dir_in(&data).join("keys");
    if !keys.join("daemon.pem").is_file() {
        pair(&keys, "daemon");
    }
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
            self.wrong.push(format!("{question}, {said}"));
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

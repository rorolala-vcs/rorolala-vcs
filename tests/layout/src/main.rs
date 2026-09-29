//! `layout`: what a layout does when more than one process changes it at once.
//!
//! A program rather than a set of tests cargo runs, because what it checks is only true of more
//! than one process: the layout is the one thing a version control changes in place, and a server
//! has several processes writing it while it stays readable. A test in one process can only meet
//! the in-process locks again; only separate processes meet the locks the operating system keeps.
//!
//! So the program runs writers and a compactor as children of itself, all working one layout, and
//! then opens it fresh to see whether every change every writer made is still there. What it is
//! really asking is the promise the layout's own documentation makes: two writers cannot tear each
//! other's records, and a compaction going on beside them neither loses what they wrote nor stops.
//!
//! The children are told what to do by the arguments they are given, so the same binary is both
//! the harness and the workload. They wait on a `go` file so that they start together, and the
//! compactor runs until a `stop` file appears, so it is still going when the writers are done.

use std::fs;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use rorolala_layout::{Layout, LayoutPath, MutableData};
use rorolala_utils_sandbox::Guard;
use uuid::Uuid;

/// How many writers change the layout at once.
const WRITERS: u8 = 4;

/// How many entries each writer makes.
const PER_WRITER: u32 = 100;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let [_, mode, rest @ ..] = args.as_slice() {
        std::process::exit(match serve(mode, rest) {
            Ok(()) => 0,
            Err(said) => {
                eprintln!("{said}");
                1
            }
        });
    }

    let sandbox = Guard::new("layout");
    let mut checked = Checked::default();

    checked.ask(
        "every entry survives writers writing at once",
        writers_alone(&sandbox),
    );
    checked.ask(
        "every entry survives a compaction going on beside the writers",
        writers_and_compaction(&sandbox),
    );

    // The sandboxes are kept rather than taken away: a run that went wrong is looked into by
    // looking at the layout it left, which a deleted directory cannot be.
    let dir = sandbox.dir().to_path_buf();
    std::mem::forget(sandbox);

    println!("layout suites left in {}", dir.display());
    checked.report();
}

/// Runs the writers over a layout of their own, and asks whether everything they wrote is there.
fn writers_alone(sandbox: &Guard) -> Result<(), String> {
    stress(sandbox, "writers-alone", false)
}

/// Runs the writers with a compactor changing the same layout beside them, and asks the same.
fn writers_and_compaction(sandbox: &Guard) -> Result<(), String> {
    stress(sandbox, "writers-and-compaction", true)
}

/// Runs the writers — and, when `compacting`, a compactor — over one layout at `sandbox/into`, and
/// reads it back whole to check nothing any writer wrote was lost.
///
/// However it ends, every child is released and waited for: a run leaves no process of its own
/// behind, so one that went wrong can still be looked into without a writer or a compactor of the
/// last one still changing the directory being looked at.
fn stress(sandbox: &Guard, into: &str, compacting: bool) -> Result<(), String> {
    let root = sandbox.join(into);
    fs::create_dir_all(&root).map_err(|error| format!("making the layout directory: {error}"))?;

    let go = sandbox.join(format!("{into}-go"));
    let stop = sandbox.join(format!("{into}-stop"));
    let _ = fs::remove_file(&go);
    let _ = fs::remove_file(&stop);

    let program = std::env::current_exe().map_err(|error| format!("finding myself: {error}"))?;

    let root_text = root.display().to_string();
    let go_text = go.display().to_string();
    let stop_text = stop.display().to_string();

    let mut children = Vec::new();
    let mut spawned = Ok(());
    for writer in 0..WRITERS {
        spawned = spawned.and_then(|()| {
            start(
                &program,
                &[
                    "writer",
                    &root_text,
                    &writer.to_string(),
                    &PER_WRITER.to_string(),
                    &go_text,
                ],
            )
            .map(|child| children.push((format!("writer {writer}"), child)))
        });
    }
    if compacting {
        spawned = spawned.and_then(|()| {
            start(&program, &["compactor", &root_text, &go_text, &stop_text])
                .map(|child| children.push(("compactor".to_owned(), child)))
        });
    }

    // Everyone starts at once, so a writer is writing while another is and while the compactor is.
    // The release comes even when a child could not be started, because one that was started is
    // waiting on it and would otherwise wait out its whole patience for nothing.
    let released =
        fs::write(&go, b"go").map_err(|error| format!("releasing the children: {error}"));
    if spawned.is_err() || released.is_err() {
        let _ = fs::write(&stop, b"stop");
        for (_, child) in children {
            let _ = child.wait_with_output();
        }
        return Err(spawned.err().or(released.err()).unwrap_or_default());
    }

    let mut failures = Vec::new();
    let mut compactor = None;
    for (what, child) in children {
        if what == "compactor" {
            compactor = Some(child);
        } else if let Err(said) = wait(child, &what) {
            failures.push(said);
        }
    }

    let stopped =
        fs::write(&stop, b"stop").map_err(|error| format!("stopping the compactor: {error}"));
    if let Some(compactor) = compactor
        && let Err(said) = wait(compactor, "compactor")
    {
        failures.push(said);
    }
    if let Err(said) = stopped {
        failures.push(said);
    }

    if !failures.is_empty() {
        return Err(failures.join("; "));
    }

    read_back(&root)
}

/// Starts `program` on `args`, collecting what it says so a child that failed can be reported.
fn start(program: &Path, args: &[&str]) -> Result<Child, String> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    command
        .spawn()
        .map_err(|error| format!("starting `{}`: {error}", args.join(" ")))
}

/// Waits for a child, and refuses to go on when it ended other than successfully.
fn wait(child: Child, what: &str) -> Result<(), String> {
    let out = child
        .wait_with_output()
        .map_err(|error| format!("waiting for {what}: {error}"))?;
    if out.status.success() {
        return Ok(());
    }

    Err(format!(
        "{what} failed: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    ))
}

/// Reads the layout at `root` back whole, and refuses it unless every writer's entries are there.
fn read_back(root: &Path) -> Result<(), String> {
    let layout =
        Layout::open(root).map_err(|error| format!("opening the layout again: {error}"))?;

    let mut wrong = Vec::new();
    for writer in 0..WRITERS {
        for k in 0..PER_WRITER {
            let id = id_of(writer, k);
            let path = LayoutPath::new(&path_of(writer, k))
                .map_err(|error| format!("`{}`: {error}", path_of(writer, k)))?;

            if layout.entry(id) != Some(expected_data(writer, k)) {
                wrong.push(format!("entry {id} is not what writer {writer} made"));
            }
            if layout.path_of(id).as_ref() != Some(&path) {
                wrong.push(format!(
                    "entry {id} is at {:?} rather than {path}",
                    layout.path_of(id)
                ));
            }
            if layout.id_of(&path) != Some(id) {
                wrong.push(format!(
                    "{path} names {:?} rather than {id}",
                    layout.id_of(&path)
                ));
            }
        }
    }

    if wrong.is_empty() {
        return Ok(());
    }

    Err(format!(
        "{} of {} entries did not come back; first: {}",
        wrong.len(),
        usize::from(WRITERS) * PER_WRITER as usize,
        wrong[0]
    ))
}

/// What a child was asked to do, run from the arguments it was given.
fn serve(mode: &str, rest: &[String]) -> Result<(), String> {
    match mode {
        "writer" => write(rest),
        "compactor" => compact(rest),
        other => Err(format!("there is no mode `{other}`")),
    }
}

/// The `writer` child: makes its entries, and a path for each, in the layout at `rest[0]`.
fn write(rest: &[String]) -> Result<(), String> {
    let (dir, rest) = rest.split_first().ok_or("writer wants a directory")?;
    let writer: u8 = rest
        .first()
        .ok_or("writer wants its number")?
        .parse()
        .map_err(|error| format!("writer's number: {error}"))?;
    let count: u32 = rest
        .get(1)
        .ok_or("writer wants how many to make")?
        .parse()
        .map_err(|error| format!("how many to make: {error}"))?;
    let go = rest.get(2).ok_or("writer wants a go file")?;

    wait_for(Path::new(go))?;
    let layout = Layout::open(dir).map_err(|error| format!("opening the layout: {error}"))?;

    for k in 0..count {
        let id = id_of(writer, k);
        layout
            .create_entry(id, expected_data(writer, k))
            .map_err(|error| format!("writing entry {id}: {error}"))?;

        let path = LayoutPath::new(&path_of(writer, k))
            .map_err(|error| format!("`{}`: {error}", path_of(writer, k)))?;
        layout
            .create_path(&path, id)
            .map_err(|error| format!("naming entry {id}: {error}"))?;
    }

    Ok(())
}

/// The `compactor` child: writes the layout down whole, over and over, until it is told to stop.
fn compact(rest: &[String]) -> Result<(), String> {
    let (dir, rest) = rest.split_first().ok_or("compactor wants a directory")?;
    let go = rest.first().ok_or("compactor wants a go file")?;
    let stop = rest.get(1).ok_or("compactor wants a stop file")?;

    wait_for(Path::new(go))?;
    let layout = Layout::open(dir).map_err(|error| format!("opening the layout: {error}"))?;

    let mut rounds = 0_u64;
    while !Path::new(stop).exists() {
        layout
            .compact()
            .map_err(|error| format!("compacting: {error}"))?;
        rounds += 1;

        // Without a breath the compactor holds the log locks nearly all the time, and the writers
        // are checked for progress rather than for how fast they make it.
        std::thread::sleep(Duration::from_millis(1));
    }

    eprintln!("compacted {rounds} times");
    Ok(())
}

/// Waits for `path` to appear, since it is what says the run has begun.
fn wait_for(path: &Path) -> Result<(), String> {
    for _ in 0..10_000 {
        if path.exists() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(2));
    }

    Err(format!("waiting for {}", path.display()))
}

/// The `Uuid` the entry a given writer made at a given number is.
///
/// Different writers never make the same one, so no two of them are ever writing about the same
/// entry — the point is what the log does with many at once, not who wins a race. The first byte is
/// what the layout sorts an entry by, so it is made to change with the number: every shard is one
/// writers are writing to, not just the one they would all land in.
fn id_of(writer: u8, k: u32) -> Uuid {
    let mut bytes = [0_u8; 16];
    bytes[0] = k as u8;
    bytes[1] = writer + 1;
    bytes[2..6].copy_from_slice(&(k + 1).to_be_bytes());

    Uuid::from_bytes(bytes)
}

/// What the entry a given writer made at a given number holds.
fn expected_data(writer: u8, k: u32) -> MutableData {
    let mut version = [0_u8; 32];
    version[0] = writer;
    version[1] = (k & 0xff) as u8;
    version[2] = ((k >> 8) & 0xff) as u8;

    MutableData::new(
        Some(format!("account-{writer}")),
        version,
        format!("writer {writer} made this at {k}"),
    )
}

/// The path the entry a given writer made at a given number is at.
fn path_of(writer: u8, k: u32) -> String {
    format!("writer-{writer}/made-at-{k}")
}

/// What was asked of the layout, and which of the questions were not answered as they should be.
#[derive(Default)]
struct Checked {
    /// How many questions were asked.
    asked: usize,
    /// Which of them were answered with something other than what was wanted.
    wrong: Vec<String>,
}

impl Checked {
    /// Counts a question, and records it as wrong when it was not answered as it should be.
    fn ask(&mut self, question: &str, answered: Result<(), String>) {
        self.asked += 1;

        if let Err(said) = answered {
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

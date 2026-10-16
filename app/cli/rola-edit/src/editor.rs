//! The editor a run draws in the terminal.
//!
//! The screen is the alternate one, so what was on the terminal before a run is there again after
//! it, and the terminal is in raw mode while it runs, so every key reaches this and none is
//! handled by the shell. What is drawn is three things: the lines, a status bar under them, and
//! one line above the status bar for a message that only has to be said once.
//!
//! A line is measured by what the terminal shows it as rather than by how many characters it has
//! — the material this is opened on is CJK, which is two columns a character — so the width of a
//! line and where the cursor sits in it are both counted in columns. See [`display_width`].

use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use crossterm::{
    QueueableCommand,
    cursor::MoveTo,
    event,
    event::{Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    style::{self, Color, Print, SetForegroundColor},
    terminal::{
        self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    },
};
use rust_i18n::t;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// A line's width in terminal columns.
#[must_use]
fn display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

/// The longest prefix of `text` that fits in `width` columns.
///
/// A character that would cross the edge is left out whole rather than cut, so a wide character is
/// never shown as half of itself. A combining mark, which has no width of its own, still costs
/// nothing and stays with the character it belongs to.
#[must_use]
fn fit(text: &str, width: usize) -> &str {
    let mut used = 0;

    for (at, character) in text.char_indices() {
        let cost = UnicodeWidthChar::width(character).unwrap_or(0);

        if used + cost > width {
            return &text[..at];
        }

        used += cost;
    }

    text
}

/// The byte offset of the character at `index`, or the end of `line`.
///
/// The cursor is counted in characters, because that is what a reader moves it by, while a line is
/// cut and written by bytes, because that is what a `String` is. Every such crossing goes through
/// here, so the two countings cannot drift apart.
#[must_use]
fn byte_at(line: &str, index: usize) -> usize {
    line.char_indices()
        .nth(index)
        .map_or(line.len(), |(at, _)| at)
}

/// `value` as a terminal coordinate, saturating at the largest one.
///
/// A coordinate wider than the terminal can name is one no terminal can show; clamping it is a
/// screen that is wrong in a way a reader can see rather than a number that wrapped.
#[must_use]
fn coord(value: usize) -> u16 {
    u16::try_from(value).unwrap_or(u16::MAX)
}

/// The editor, as it stands between two keys.
///
/// Its own state is a handful of independent facts — whether the text has been changed, whether a
/// warning was shown, whether the run is ending, and on Windows whether an IME is composing — and
/// a state machine over them would be more machinery than they are worth here.
#[allow(clippy::struct_excessive_bools)]
pub struct Editor {
    /// The lines of the file, as they stand now.
    lines: Vec<String>,
    /// The cursor's column, counted in characters.
    x: usize,
    /// The cursor's row, counted in lines.
    y: usize,
    /// The file being edited.
    path: PathBuf,
    /// Whether what is on screen differs from what is on disk.
    modified: bool,
    /// The terminal's width and height, as last measured.
    size: (u16, u16),
    /// Whether the run is to end.
    exit: bool,
    /// Whether the unsaved warning has already been shown, so the next Esc leaves.
    warned: bool,
    /// What was last said to the caller, shown until the next key is pressed.
    message: Option<String>,
    /// The last key seen, and when, for the doubled events a Windows terminal sends.
    #[cfg(windows)]
    last: Option<(KeyCode, KeyModifiers, std::time::Instant)>,
    /// Whether an IME is composing, on Windows, where one is between the keyboard and this.
    #[cfg(windows)]
    composing: bool,
}

impl Editor {
    /// Opens `path`, reading the lines it holds.
    ///
    /// # Errors
    ///
    /// Returns the reason the file could not be read.
    pub fn open(path: PathBuf) -> io::Result<Self> {
        let mut lines: Vec<String> = fs::read_to_string(&path)?
            .lines()
            .map(str::to_owned)
            .collect();

        // There is always one line to stand on, even in an empty file, so nothing below has to
        // ask whether there is a line at all.
        if lines.is_empty() {
            lines.push(String::new());
        }

        Ok(Self {
            lines,
            x: 0,
            y: 0,
            path,
            modified: false,
            size: terminal::size().unwrap_or((80, 24)),
            exit: false,
            warned: false,
            message: None,
            #[cfg(windows)]
            last: None,
            #[cfg(windows)]
            composing: false,
        })
    }

    /// Runs the editor to its end, leaving the terminal as it was found.
    ///
    /// The terminal is put right whether the run ended well or badly: a run that failed and left
    /// the shell in raw mode would have broken the very thing it failed in.
    ///
    /// # Errors
    ///
    /// Returns the reason the terminal could not be prepared, or the reason a draw or a read
    /// failed while the editor ran.
    pub fn run(&mut self) -> io::Result<()> {
        let mut out = io::stdout();

        enable_raw_mode()?;

        if let Err(error) = execute!(out, EnterAlternateScreen) {
            // Raw mode was already on when this failed, so it is turned back off here.
            disable_raw_mode().ok();

            return Err(error);
        }

        // Keys typed while the shell was still reading are none of this run's, and a leftover
        // Enter would otherwise be one of its first.
        Self::drain_input();

        let result = self.event_loop(&mut out);

        let _ = execute!(out, LeaveAlternateScreen);
        let _ = disable_raw_mode();

        result
    }

    /// Reads keys until the run is asked to end.
    fn event_loop(&mut self, out: &mut io::Stdout) -> io::Result<()> {
        self.render(out)?;

        loop {
            match event::read()? {
                Event::Key(key) => {
                    if self.ignored(&key) {
                        continue;
                    }

                    self.handle_key(key, out)?;

                    if self.exit {
                        return Ok(());
                    }
                }
                Event::Resize(width, height) => {
                    self.size = (width, height);
                    self.render(out)?;
                }
                _ => {}
            }
        }
    }

    /// Whether a key is one to pass over rather than act on.
    ///
    /// Windows sends a key twice — once as a press and once as a release — and sends its own noise
    /// around an IME's composition, so both are told apart from a keystroke there. Nowhere else has
    /// either to answer for, and the answer is always no.
    #[cfg(windows)]
    fn ignored(&mut self, key: &KeyEvent) -> bool {
        use crossterm::event::KeyEventKind;

        if matches!(key.kind, KeyEventKind::Release) {
            return true;
        }

        self.ime_noise(key) || self.doubled(key)
    }

    /// Leaves the key as it is: no other platform sends a press and a release, or composes.
    #[cfg(not(windows))]
    #[allow(
        clippy::unused_self,
        clippy::needless_pass_by_ref_mut,
        reason = "the signature stands in for the Windows one it replaces, which needs `self`"
    )]
    fn ignored(&mut self, _key: &KeyEvent) -> bool {
        false
    }

    /// Whether a key is the same one just seen, sent twice.
    #[cfg(windows)]
    fn doubled(&mut self, key: &KeyEvent) -> bool {
        let now = std::time::Instant::now();
        let seen = (key.code, key.modifiers, now);

        if let Some((code, modifiers, when)) = &self.last
            && *code == key.code
            && *modifiers == key.modifiers
            && now.duration_since(*when) < std::time::Duration::from_millis(20)
        {
            return true;
        }

        self.last = Some(seen);
        false
    }

    /// Whether a key is the noise an IME makes for itself rather than input.
    #[cfg(windows)]
    fn ime_noise(&mut self, key: &KeyEvent) -> bool {
        let KeyCode::Char(character) = key.code else {
            return false;
        };

        let point = u32::from(character);

        // The markers some IMEs bracket a composition with.
        if point == 0x16 || point == 0x17 || point == 0x18 {
            self.composing = true;
            return true;
        }

        // Control characters and C1 control codes are never text to show.
        if point < 0x20 || (0x80..0xA0).contains(&point) {
            return true;
        }

        // A composition that has produced a printable character is over.
        if self.composing {
            if character.is_ascii_graphic() || character.is_alphanumeric() {
                self.composing = false;
            } else {
                return true;
            }
        }

        false
    }

    /// Acts on one key, and draws the result.
    fn handle_key(&mut self, key: KeyEvent, out: &mut io::Stdout) -> io::Result<()> {
        // Any key but an Esc means the run is no longer hesitating to leave, so a warning that was
        // shown no longer stands; and what was last said is said once, so it goes with the key.
        if !matches!(key.code, KeyCode::Esc) {
            self.warned = false;
            self.message = None;
        }

        match key.code {
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.save();
            }
            // Every other control chord is a command this editor does not have, and the letter
            // under it is not text the caller typed.
            KeyCode::Char(_) if key.modifiers.contains(KeyModifiers::CONTROL) => {}
            KeyCode::Char(character) => self.insert(character),
            KeyCode::Enter => self.open_line(),
            KeyCode::Backspace => self.backspace(),
            KeyCode::Left => self.walk(-1, 0),
            KeyCode::Right => self.walk(1, 0),
            KeyCode::Up => self.walk(0, -1),
            KeyCode::Down => self.walk(0, 1),
            KeyCode::Esc => self.escape(),
            _ => {}
        }

        self.render(out)
    }

    /// Writes the lines back to the file, and says so.
    fn save(&mut self) {
        match fs::write(&self.path, self.lines.join("\n")) {
            Ok(()) => {
                self.modified = false;
                self.notice(t!("edit.messages.file_saved"));
            }
            Err(error) => eprintln!("{}", t!("edit.errors.save", error = error)),
        }
    }

    /// Ends the run, or asks a second time when there is unsaved work.
    fn escape(&mut self) {
        if !self.modified || self.warned {
            self.exit = true;

            return;
        }

        self.warned = true;
        self.notice(t!("edit.messages.unsaved_changes"));
    }

    /// Moves the cursor by `dx` columns and `dy` lines, staying within the text.
    ///
    /// A move across lines lands at the column asked for, or at the end of the line when the line
    /// is shorter than that column; the column is not remembered, so a step onto a short line and
    /// off it again ends at the short line's own end.
    fn walk(&mut self, dx: isize, dy: isize) {
        if dy != 0 {
            self.y = self.y.saturating_add_signed(dy).min(self.lines.len() - 1);
            self.x = self.x.min(self.line_length());
        } else if dx != 0 {
            self.x = self.x.saturating_add_signed(dx).min(self.line_length());
        }
    }

    /// Puts `character` where the cursor is.
    fn insert(&mut self, character: char) {
        let line = &mut self.lines[self.y];
        let at = byte_at(line, self.x);

        line.insert(at, character);

        self.x += 1;
        self.modified = true;
    }

    /// Splits the line at the cursor, leaving the rest on a line of its own.
    fn open_line(&mut self) {
        let line = self.lines[self.y].clone();
        let at = byte_at(&line, self.x);
        let rest = line[at..].to_owned();

        self.lines[self.y].truncate(at);
        self.lines.insert(self.y + 1, rest);

        self.y += 1;
        self.x = 0;
        self.modified = true;
    }

    /// Removes the character before the cursor, or joins the line onto the one above.
    fn backspace(&mut self) {
        if self.x > 0 {
            let line = &mut self.lines[self.y];
            let start = byte_at(line, self.x - 1);
            let end = byte_at(line, self.x);

            line.replace_range(start..end, "");

            self.x -= 1;
            self.modified = true;
        } else if self.y > 0 {
            let rest = self.lines.remove(self.y);
            self.y -= 1;
            self.x = self.line_length();
            self.lines[self.y].push_str(&rest);
            self.modified = true;
        }
    }

    /// How many characters the cursor's line has.
    fn line_length(&self) -> usize {
        self.lines[self.y].chars().count()
    }

    /// The first line shown, which follows the cursor down the screen.
    fn first_shown(&self) -> usize {
        let shown = self.visible_rows();
        self.y.saturating_add(1).saturating_sub(shown)
    }

    /// How many lines fit above the status bar and the message line.
    ///
    /// One line is always left, so a terminal too short to hold any is drawn onto rather than
    /// divided by zero.
    fn visible_rows(&self) -> usize {
        usize::from(self.size.1.saturating_sub(2)).max(1)
    }

    /// Draws the lines, the message, the status bar, and the cursor.
    fn render(&self, out: &mut io::Stdout) -> io::Result<()> {
        let width = usize::from(self.size.0);
        let first = self.first_shown();
        let last = (first + self.visible_rows()).min(self.lines.len());

        out.queue(Clear(ClearType::All))?;

        for (row, line) in self.lines[first..last].iter().enumerate() {
            out.queue(MoveTo(0, coord(row)))?;
            out.queue(Print(fit(line, width)))?;
        }

        if let Some(message) = &self.message {
            self.draw_message(out, message)?;
        }

        self.draw_status(out)?;
        self.move_cursor(out, first)?;

        out.flush()
    }

    /// Draws what was last said along the line above the status bar.
    fn draw_message(&self, out: &mut io::Stdout, message: &str) -> io::Result<()> {
        let width = usize::from(self.size.0);
        let row = self.size.1.saturating_sub(2);

        out.queue(MoveTo(0, row))?;
        out.queue(Clear(ClearType::CurrentLine))?;
        out.queue(SetForegroundColor(Color::Yellow))?;
        out.queue(style::SetBackgroundColor(Color::DarkBlue))?;
        out.queue(Print(fit(message, width)))?;
        out.queue(style::ResetColor)?;

        Ok(())
    }

    /// Draws the status bar along the bottom line.
    fn draw_status(&self, out: &mut io::Stdout) -> io::Result<()> {
        let width = usize::from(self.size.0);
        let row = self.size.1.saturating_sub(1);
        let modified = if self.modified {
            t!("edit.messages.modified")
        } else {
            "".into()
        };

        let status = format!(
            "{} - {} {}{} {}",
            self.path.display(),
            self.lines.len(),
            t!("edit.status.lines"),
            modified,
            t!("edit.hints"),
        );

        out.queue(MoveTo(0, row))?;
        out.queue(Clear(ClearType::CurrentLine))?;
        out.queue(SetForegroundColor(Color::Black))?;
        out.queue(style::SetBackgroundColor(Color::White))?;
        out.queue(Print(fit(&status, width)))?;
        out.queue(style::ResetColor)?;

        Ok(())
    }

    /// Puts the cursor where it belongs, counted from the first line shown.
    fn move_cursor(&self, out: &mut io::Stdout, first: usize) -> io::Result<()> {
        let width = usize::from(self.size.0);
        let line = &self.lines[self.y];
        let column = display_width(&line[..byte_at(line, self.x)]);

        out.queue(MoveTo(coord(column.min(width)), coord(self.y - first)))?;

        Ok(())
    }

    /// Says one thing, on the line above the status bar, until the next key is pressed.
    fn notice(&mut self, message: impl Into<String>) {
        self.message = Some(message.into());
    }

    /// Throws away any key already waiting to be read.
    fn drain_input() {
        while event::poll(std::time::Duration::ZERO).unwrap_or(false) {
            if event::read().is_err() {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{byte_at, fit};

    #[test]
    fn a_byte_offset_lands_on_a_character() {
        assert_eq!(byte_at("abc", 0), 0);
        assert_eq!(byte_at("abc", 2), 2);
        assert_eq!(byte_at("abc", 3), 3);
        // Past the end is the end, so a cursor one step too far still points at a boundary.
        assert_eq!(byte_at("abc", 9), 3);
        // A character outside ascii takes more than one byte.
        assert_eq!(byte_at("一a", 1), 3);
    }

    #[test]
    fn a_line_is_cut_to_what_fits() {
        assert_eq!(fit("abc", 2), "ab");
        assert_eq!(fit("abc", 9), "abc");
        // A wide character that would cross the edge is left out whole.
        assert_eq!(fit("一", 1), "");
        assert_eq!(fit("a一", 2), "a");
    }
}

//! The version chain and how it is drawn.
//!
//! A [`Chain`] is the top of a version chain and every variant under it, the variants that form the
//! spine — one per level — and those merged into them. It says nothing about where anything is
//! drawn: [`draw`] turns it into a grid of cells, where a cell says whether it is a version's
//! number, a variant's label or an edge and the caller decides what each looks like. That keeps the
//! look of the drawing — which is a terminal's business — out of the shape of the graph.
//!
//! The root version is not part of a chain: the chain stops at the first version, so every level is
//! zero or more. The top may be a version with a number, or — when a single variant is looked back
//! from — the version that variant produced, whose number is drawn `?`: what a variant is drawn
//! between is the version it produced and the one it is based on, so looking back from a variant
//! shows the same chain with an unnamed version at the top — see [`Chain::unknown`].

/// What the top of a chain is, as it is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Top {
    /// The highest version's number, drawn `V{n}`.
    Numbered,
    /// A version whose number is unknown, drawn `?`.
    ///
    /// It is the top a Variant is looked back from: the variant is the one the highest version was
    /// produced by, but which version that is takes a search of the whole index to find, and a
    /// drawing does not know it.
    Unknown,
    /// A version being edited, drawn `??`.
    ///
    /// It is the top a file with changes that were never recorded is looked back from: the version
    /// the work is on its way to is not one the index has, and neither is the variant that would
    /// produce it, so both are drawn as placeholders over the version being edited.
    Editing,
}

/// A chain of versions, top to bottom.
pub struct Chain {
    /// The number of the highest version drawn.
    top: u64,
    /// What the highest version is, as it is drawn.
    top_kind: Top,
    /// Every variant drawn, spine and merged alike.
    nodes: Vec<Node>,
}

impl Chain {
    /// A chain whose highest version is at `top`, over `nodes`.
    #[must_use]
    pub const fn new(top: u64, top_kind: Top, nodes: Vec<Node>) -> Self {
        Self {
            top,
            top_kind,
            nodes,
        }
    }

    /// The number of the highest version drawn.
    #[must_use]
    pub const fn top(&self) -> u64 {
        self.top
    }

    /// What the highest version is, as it is drawn.
    #[must_use]
    pub const fn top_kind(&self) -> Top {
        self.top_kind
    }

    /// Every variant drawn.
    #[must_use]
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }
}

/// One variant of a [`Chain`].
pub struct Node {
    /// The label it is drawn by: a variant's hash, shortened to its leading characters.
    label: String,
    /// The number of the version it is based on, which is the level it sits at.
    level: u64,
    /// Whether it is the spine variant of its level: the one the version above points at.
    spine: bool,
    /// The variant it merges in, as an index into the chain's nodes.
    join: Option<usize>,
}

impl Node {
    /// A node drawn by `label` at `level`.
    #[must_use]
    pub const fn new(label: String, level: u64, spine: bool, join: Option<usize>) -> Self {
        Self {
            label,
            level,
            spine,
            join,
        }
    }

    /// The label it is drawn by.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The level it sits at: the number of the version it is based on.
    #[must_use]
    pub const fn level(&self) -> u64 {
        self.level
    }

    /// Whether it is the spine variant of its level.
    #[must_use]
    pub const fn is_spine(&self) -> bool {
        self.spine
    }

    /// The variant it merges in, as an index into the chain's nodes.
    #[must_use]
    pub const fn join(&self) -> Option<usize> {
        self.join
    }
}

/// How many columns a variant's label takes.
pub const LABEL_WIDTH: usize = 7;

/// The column a version's edge drops by, just right of the number.
///
/// It is the same for every level whatever the number is: a chain whose numbers grow a digit does
/// not shift the path down the left, it only widens the numbers beside it.
const INDENT: usize = 2;

/// The column a spine variant's label starts at.
const SPINE: usize = INDENT + 1;

/// How far apart the labels of one level are laid out.
const STEP: usize = LABEL_WIDTH + 3;

/// What a drawn cell stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// A version's number.
    Version,
    /// A variant's label.
    Variant,
    /// An edge between them.
    Edge,
}

/// A cell of a drawing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    /// Nothing is drawn here.
    Blank,
    /// A character, and what it stands for.
    Glyph(Role, char),
}

/// A drawing: what [`draw`] lays out.
///
/// What a caller is given is the grid and, beside it, where each version's number ended up: a
/// version's row is where a reader says what that version was, and a caller that wants to write
/// beside the number would otherwise have to read the number back out of the glyphs.
pub struct Drawing {
    /// The rows, the topmost first.
    rows: Vec<Vec<Cell>>,
    /// The row each version's number is drawn on, the highest first.
    versions: Vec<VersionRow>,
}

impl Drawing {
    /// The rows, the topmost first.
    #[must_use]
    pub fn rows(&self) -> &[Vec<Cell>] {
        &self.rows
    }

    /// The row each version's number is drawn on, the highest first.
    #[must_use]
    pub fn versions(&self) -> &[VersionRow] {
        &self.versions
    }
}

/// Where one version's number is drawn.
pub struct VersionRow {
    /// The number of the version, which is the level it is drawn at.
    level: u64,
    /// The row its number is on.
    row: usize,
}

impl VersionRow {
    /// The number of the version.
    #[must_use]
    pub const fn level(&self) -> u64 {
        self.level
    }

    /// The row its number is on.
    #[must_use]
    pub const fn row(&self) -> usize {
        self.row
    }
}

/// Draws `chain`.
///
/// The top is a version's number — the highest one, or `?` when that number is unknown. Under it
/// the chain descends level by level to the first version, numbered zero; the root, which the first
/// version's variant is based on, is left out. Each level holds its spine variant where the version
/// above drops to it, and the variants merged into the chain to its right, each reached by a bus
/// that runs down from the variant that merged it in.
#[must_use]
pub fn draw(chain: &Chain) -> Drawing {
    let top = chain.top();
    let Ok(levels) = usize::try_from(top) else {
        return Drawing {
            rows: Vec::new(),
            versions: Vec::new(),
        };
    };

    let rows = Rows::of(top);
    let node_col = columns(chain, top);
    let mut grid = Grid::new(rows.height);

    paint_versions(&mut grid, levels, &rows, chain);
    paint_labels(&mut grid, chain.nodes(), &rows, &node_col);
    paint_spine(&mut grid, levels, &rows);
    paint_feet(&mut grid, chain, levels, &rows, &node_col);
    paint_joins(&mut grid, chain, &rows, &node_col);

    let versions = (0..=levels)
        .rev()
        .filter_map(|level| {
            let level = u64::try_from(level).unwrap_or(0);
            let row = Rows::at(&rows.version, level)?;

            Some(VersionRow { level, row })
        })
        .collect();

    Drawing {
        rows: grid.cells,
        versions,
    }
}

/// The rows each level owns.
struct Rows {
    /// The row each version's number is drawn on.
    version: Vec<Option<usize>>,
    /// The row the spine's up edge is drawn on, when there is an up edge.
    up: Vec<Option<usize>>,
    /// The row each level's variants are drawn on.
    variant: Vec<Option<usize>>,
    /// The row the level's feet are drawn on.
    down: Vec<Option<usize>>,
    /// How many rows the drawing has.
    height: usize,
}

impl Rows {
    /// Lays the levels out.
    ///
    /// The highest version is drawn first, then for each level under it the edge down, the
    /// variants, the edge on, and the version below. A version whose own row is a placeholder —
    /// looked back from a variant, or being edited — has its row where its number would be.
    fn of(top: u64) -> Self {
        let Ok(levels) = usize::try_from(top) else {
            return Self {
                version: Vec::new(),
                up: Vec::new(),
                variant: Vec::new(),
                down: Vec::new(),
                height: 0,
            };
        };
        let size = levels + 1;
        let mut rows = Self {
            version: vec![None; size],
            up: vec![None; size],
            variant: vec![None; size],
            down: vec![None; size],
            height: 0,
        };

        rows.version[levels] = Some(0);

        for step in 0..levels {
            let level = levels - 1 - step;
            let base = 1 + 4 * step;
            rows.up[level] = Some(base);
            rows.variant[level] = Some(base + 1);
            rows.down[level] = Some(base + 2);
            rows.version[level] = Some(base + 3);
        }

        rows.height = 1 + 4 * levels;

        rows
    }

    /// The row `table` has for `level`.
    fn at(table: &[Option<usize>], level: u64) -> Option<usize> {
        table.get(usize::try_from(level).ok()?).copied().flatten()
    }
}

/// The column each variant's label starts at.
///
/// The spine sits one step past the indent. The merged variants of a level are laid out to its
/// right, one step apart, the one merged in latest furthest out.
fn columns(chain: &Chain, top: u64) -> Vec<usize> {
    let nodes = chain.nodes();
    let mut node_col = vec![0; nodes.len()];

    for level in levels_of(top) {
        for (index, node) in nodes.iter().enumerate() {
            if node.level() == level && node.is_spine() {
                node_col[index] = SPINE;
            }
        }

        for (order, index) in merged(chain, level).into_iter().enumerate() {
            node_col[index] = SPINE + STEP * (order + 1);
        }
    }

    node_col
}

/// The version numbers, down the left.
///
/// The highest version's row is a number when it has one, and a placeholder when it does not: `?`
/// for a version nothing names, `??` for one being edited.
fn paint_versions(grid: &mut Grid, levels: usize, rows: &Rows, chain: &Chain) {
    let top = chain.top();

    for level in 0..=levels {
        let level = u64::try_from(level).unwrap_or(0);
        let Some(row) = Rows::at(&rows.version, level) else {
            continue;
        };

        let number = if level == top {
            match chain.top_kind() {
                Top::Numbered => format!("V{level}"),
                Top::Unknown => "?".to_owned(),
                Top::Editing => "??".to_owned(),
            }
        } else {
            format!("V{level}")
        };

        grid.text(row, 0, Role::Version, &number);
    }
}

/// The variant labels.
fn paint_labels(grid: &mut Grid, nodes: &[Node], rows: &Rows, node_col: &[usize]) {
    for (index, node) in nodes.iter().enumerate() {
        let Some(row) = Rows::at(&rows.variant, node.level()) else {
            continue;
        };
        grid.text(row, node_col[index], Role::Variant, node.label());
    }
}

/// The spine's own edges: from the version above down to its variant, and on from the variant to
/// the version below.
fn paint_spine(grid: &mut Grid, levels: usize, rows: &Rows) {
    for level in 0..=levels {
        let level = u64::try_from(level).unwrap_or(0);

        if let Some(row) = Rows::at(&rows.up, level) {
            grid.edge(row, INDENT, '\\');
        }
        if let Some(row) = Rows::at(&rows.down, level) {
            grid.edge(row, INDENT, '/');
        }
    }
}

/// What the variants of a level stand on: one foot each, drawn as a bar when several share the
/// level, all reaching down to the base version below.
fn paint_feet(grid: &mut Grid, chain: &Chain, levels: usize, rows: &Rows, node_col: &[usize]) {
    for level in 0..=levels {
        let level = u64::try_from(level).unwrap_or(0);
        let Some(row) = Rows::at(&rows.down, level) else {
            continue;
        };
        let mut feet = vec![INDENT];
        for index in merged(chain, level) {
            feet.push(node_col[index].saturating_sub(1));
        }
        if feet.len() == 1 {
            continue;
        }

        feet.sort_unstable();
        let left = feet[0];
        let right = *feet.last().unwrap_or(&left);
        for column in left..=right {
            if feet.contains(&column) {
                grid.edge(row, column, '/');
            } else {
                grid.edge(row, column, '_');
            }
        }
    }
}

/// What each merging variant is tied to.
///
/// The line leaves under its label, on the row below it, and runs down to the column the merged
/// variant is drawn in: diagonally a column a row while it can, then straight down, the turn marked
/// with a star. When the merged variant is too far out for a diagonal to reach in the rows there
/// are, the line first runs across a label's width on the row below its own, then carries on.
fn paint_joins(grid: &mut Grid, chain: &Chain, rows: &Rows, node_col: &[usize]) {
    let nodes = chain.nodes();

    for (index, node) in nodes.iter().enumerate() {
        let Some(target) = node.join() else {
            continue;
        };
        let Some(tied) = nodes.get(target) else {
            continue;
        };
        let (Some(start), Some(end)) = (
            Rows::at(&rows.variant, node.level()),
            Rows::at(&rows.variant, tied.level()),
        ) else {
            continue;
        };
        let row_from = start + 1;
        if end <= row_from {
            continue;
        }

        let to = node_col[target];
        let from = node_col[index] + LABEL_WIDTH;

        // A merged variant under its own column, or to the left, is reached straight down.
        if to <= from {
            for row in row_from..end {
                grid.cross(row, to, '|');
            }
            continue;
        }

        // A diagonal gains a column a row, so a target further out than that needs the extra room
        // taken on one row rather than a run of them.
        let detour = from + (end - row_from) < to;
        let mut row = row_from;
        let mut column = from;

        loop {
            if column >= to {
                if row < end {
                    grid.edge(row, to, '*');
                    for below in (row + 1)..end {
                        grid.cross(below, to, '|');
                    }
                }
                break;
            }
            if row >= end {
                break;
            }
            if detour && row == row_from + 3 {
                grid.edge(row, column, '*');
                for across in 1..=LABEL_WIDTH {
                    grid.edge(row, column + across, '_');
                }
                column += LABEL_WIDTH + 1;
                row += 1;
                continue;
            }
            grid.cross(row, column, '\\');
            row += 1;
            column += 1;
        }
    }
}

/// The levels a drawing of `top` has, highest first.
///
/// The highest version has one level under it for each version down to the first, so levels
/// `top - 1 ..= 0`; a chain of one version has no level at all.
fn levels_of(top: u64) -> Vec<u64> {
    if top == 0 {
        return Vec::new();
    }

    (0..top).rev().collect()
}

/// The merged variants of `level`, left to right as the chain gives them.
fn merged(chain: &Chain, level: u64) -> Vec<usize> {
    chain
        .nodes()
        .iter()
        .enumerate()
        .filter(|(_, node)| node.level() == level && !node.is_spine())
        .map(|(index, _)| index)
        .collect()
}

/// A grid a drawing is laid out on.
struct Grid {
    /// The rows of cells.
    cells: Vec<Vec<Cell>>,
}

impl Grid {
    /// An empty grid of `rows` rows, each as short as it can be.
    fn new(rows: usize) -> Self {
        Self {
            cells: vec![Vec::new(); rows],
        }
    }

    /// Puts `cell` at `row` and `column`, growing the row to reach it.
    fn put(&mut self, row: usize, column: usize, cell: Cell) {
        let Some(line) = self.cells.get_mut(row) else {
            return;
        };

        if line.len() <= column {
            line.resize(column + 1, Cell::Blank);
        }
        line[column] = cell;
    }

    /// Writes `text` at `row` and `column` as cells of `role`.
    fn text(&mut self, row: usize, column: usize, role: Role, text: &str) {
        for (offset, glyph) in text.chars().enumerate() {
            self.put(row, column + offset, Cell::Glyph(role, glyph));
        }
    }

    /// Puts an edge glyph, replacing whatever edge was there.
    fn edge(&mut self, row: usize, column: usize, glyph: char) {
        self.put(row, column, Cell::Glyph(Role::Edge, glyph));
    }

    /// Puts an edge glyph, but never over a label, and marking a crossing with a star.
    fn cross(&mut self, row: usize, column: usize, glyph: char) {
        match self
            .cells
            .get(row)
            .and_then(|line| line.get(column))
            .copied()
        {
            Some(Cell::Glyph(Role::Variant | Role::Version, _) | Cell::Glyph(Role::Edge, '*')) => {}
            Some(Cell::Glyph(Role::Edge, there)) if there != glyph => {
                self.put(row, column, Cell::Glyph(Role::Edge, '*'));
            }
            _ => self.edge(row, column, glyph),
        }
    }
}

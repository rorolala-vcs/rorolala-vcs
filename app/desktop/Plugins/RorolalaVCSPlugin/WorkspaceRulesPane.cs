using Avalonia;
using Avalonia.Animation;
using Avalonia.Controls;
using Avalonia.Controls.Documents;
using Avalonia.Controls.Primitives;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using Avalonia.Threading;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;
using Path = Avalonia.Controls.Shapes.Path;

namespace RorolalaVCSPlugin;

/// <summary>
/// The left pane of the Workspace dock: the ignore rules, as a list and as the file they are.
/// </summary>
/// <remarks>
/// The file is the truth: a line is a pattern the loader applies, a run of <c>#</c> lines is one note,
/// and the order of the lines is the order the loader reads them in, the last rule that matches a path
/// deciding over the ones above it. The list is a reading of that file, and every change it makes is
/// made to the lines and written back, so what the next reading ignores is what this pane shows.
/// <para>
/// A note is one row rather than one row a line, and it is dragged, edited and deleted whole: a note
/// is read as prose, and the <c>#</c> marks are the file's spelling rather than the reader's content.
/// The rules a Vault handed down are shown above and are read only, since a fetch writes them.
/// </para>
/// </remarks>
internal sealed class WorkspaceRulesPane : UserControl
{
    /// <summary>How tall the pane's heading is.</summary>
    private const double Head = 36;

    /// <summary>How tall the strip a rule is added with is.</summary>
    private const double Adder = 48;

    /// <summary>How wide the word at the strip's end is.</summary>
    private const double AddWidth = 96;

    /// <summary>How tall one line of a note is.</summary>
    private const double NoteLine = 17;

    /// <summary>The room the pane keeps at its left, which is where a row's mark begins.</summary>
    private const double Edge = 16;

    /// <summary>The room a row keeps at its right.</summary>
    private const double Trail = 12;

    /// <summary>How far a note's text and a Vault's rules are indented from the pane's edge.</summary>
    private const double Indent = 34;

    /// <summary>The host, whose library holds the marks and whose dialogs show a failure.</summary>
    private readonly IPluginHost _host;

    /// <summary>Where a rule file that could not be read is said.</summary>
    private readonly ILog _log;

    /// <summary>The number of patterns beside the heading.</summary>
    private readonly TextBlock _count;

    /// <summary>What opens the file as text, and closes it again.</summary>
    private readonly Button _textMode;

    /// <summary>What the file's text is edited in.</summary>
    private readonly TextBox _editorText;

    /// <summary>The file's path over the text editor.</summary>
    private readonly TextBlock _editorFile;

    /// <summary>What writes the text back.</summary>
    private readonly Button _save;

    /// <summary>What leaves the editor without writing.</summary>
    private readonly Button _revert;

    /// <summary>The list of rules and notes, and the Vault rules above them.</summary>
    private readonly ScrollViewer _scroll;

    /// <summary>Where the rules are drawn, one row per block.</summary>
    private readonly StackPanel _rules;

    /// <summary>The blocks the rows of the list were drawn from, in the list's order.</summary>
    private IReadOnlyList<RuleBlock> _shown = [];

    /// <summary>The whole section of rules a Vault handed down, hidden when there are none.</summary>
    private readonly Border _vaults;

    /// <summary>What each Vault's rules are drawn in.</summary>
    private readonly StackPanel _vaultRows;

    /// <summary>The number of Vaults beside the section's heading.</summary>
    private readonly TextBlock _vaultsCount;

    /// <summary>The caret the section is opened and closed by.</summary>
    private readonly Path _vaultsCaret;

    /// <summary>What the section's rows fold into.</summary>
    private readonly StackPanel _vaultsBody;

    /// <summary>The transitions the section's rows hold.</summary>
    private readonly Transitions _vaultsMotion = WorkspaceFold.Motion();

    /// <summary>The strip a pattern or a note is added with.</summary>
    private readonly Border _adder;

    /// <summary>What is typed into the strip.</summary>
    private readonly TextBox _input;

    /// <summary>The word at the strip's end.</summary>
    private readonly Button _add;

    /// <summary>The word on the add button, which turns quiet while there is nothing to add.</summary>
    private readonly TextBlock _addWord;

    /// <summary>The whole text editor, shown in place of the list and the strip.</summary>
    private readonly Grid _editor;

    /// <summary>The rules as they were read, or nothing when the file could not be read.</summary>
    private WorkspaceRules? _reading;

    /// <summary>Whether the rules have been read at all, which the first paint is not.</summary>
    private bool _read;

    /// <summary>The lines being shown, in the order the loader applies them.</summary>
    private IReadOnlyList<string> _lines = [];

    /// <summary>The line the note being edited begins on, or nothing when none is.</summary>
    private int? _editingComment;

    /// <summary>Whether the section of Vault rules is open.</summary>
    private bool _vaultsOpen;

    /// <summary>Whether the file is being edited as text.</summary>
    private bool _text;

    /// <summary>The block being dragged, the line it was pressed on, and the row it is drawn by.</summary>
    private (int Start, int End, Border Row)? _drag;

    /// <summary>The row the dragged block would land against, and which side of it.</summary>
    private (RuleBlock Block, Border Row, bool After)? _drop;

    /// <summary>Makes the pane.</summary>
    /// <param name="host">The host, whose library holds the marks and whose dialogs show a failure.</param>
    /// <param name="log">Where a rule file that could not be read is said.</param>
    public WorkspaceRulesPane(IPluginHost host, ILog log)
    {
        ArgumentNullException.ThrowIfNull(host);
        ArgumentNullException.ThrowIfNull(log);

        _host = host;
        _log = log;

        _count = WorkspaceFurniture.Count(0);
        _textMode = WorkspaceFurniture.Square(
            host,
            Icons.Code,
            "rorolala_vcs.workspace.rules_text",
            () => SetText(!_text)
        );

        _vaultsCaret = WorkspaceFold.Caret(false);
        _vaultsCount = WorkspaceFurniture.Count(0);
        _vaultRows = new StackPanel();
        _vaultsBody = new StackPanel
        {
            ClipToBounds = true,
            MaxHeight = 0,
            Opacity = 0,
            Transitions = _vaultsMotion,
            Children = { _vaultRows },
        };
        _vaults = Section(_vaultsBody);

        _rules = new StackPanel();
        var body = new StackPanel { Children = { _vaults, _rules } };

        _scroll = new ScrollViewer
        {
            Content = body,
            HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled,
            VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
        };

        _input = Field();
        _addWord = new TextBlock
        {
            Text = RolaI18N.Get("rorolala_vcs.workspace.rules_add").ToUpperInvariant(),
            FontSize = 11,
            FontWeight = FontWeight.SemiBold,
            LetterSpacing = 0.9,
            VerticalAlignment = VerticalAlignment.Center,
        };
        _add = Word();
        _adder = Strip();

        _editorFile = new TextBlock
        {
            FontFamily = GraphMeasure.Mono,
            FontSize = 10.5,
            VerticalAlignment = VerticalAlignment.Center,
        };
        _editorFile[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(PaletteKeys.Faint);

        var fileBar = new Border
        {
            Height = 24,
            BorderThickness = new Thickness(0, 0, 0, 1),
            Child = new Border { Padding = new Thickness(Edge, 0), Child = _editorFile },
        };
        fileBar[!Border.BorderBrushProperty] = new DynamicResourceExtension(PaletteKeys.Splitter);

        _editorText = Editor();
        _revert = Ghost("rorolala_vcs.workspace.rules_revert", () => SetText(false));
        _save = Primary("rorolala_vcs.workspace.rules_save", SaveText);

        var composer = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 6,
            Margin = new Thickness(Edge, 10, Trail, 12),
            Children = { _revert, _save },
        };

        _editor = new Grid
        {
            RowDefinitions = new RowDefinitions("24,*,Auto"),
            IsVisible = false,
            Children = { fileBar, _editorText, composer },
        };
        Grid.SetRow(_editorText, 1);
        Grid.SetRow(composer, 2);

        var head = new Border
        {
            Height = Head,
            Padding = new Thickness(Edge, 0, Trail, 0),
            Child = Heading(),
        };

        var pane = new Grid { RowDefinitions = new RowDefinitions("Auto,*,Auto") };
        pane.Children.Add(head);
        pane.Children.Add(_scroll);
        pane.Children.Add(_adder);
        pane.Children.Add(_editor);
        Grid.SetRow(_scroll, 1);
        Grid.SetRow(_adder, 2);
        Grid.SetRow(_editor, 1);
        Grid.SetRowSpan(_editor, 2);

        Content = pane;

        PaintAdd();
        Draw();
    }

    /// <summary>Reads the rules of a Workspace and draws them.</summary>
    /// <param name="root">The Workspace's root.</param>
    public void Load(string root)
    {
        ArgumentNullException.ThrowIfNull(root);

        _editingComment = null;
        _drag = null;
        _drop = null;
        _read = true;

        try
        {
            _reading = WorkspaceRules.Read(root);
            _lines = _reading.Lines;
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            _reading = null;
            _lines = [];
            _log.Warn($"the ignore rules of `{root}` could not be read: {error.Message}");
        }

        _editorFile.Text = _reading?.Shown ?? string.Empty;
        _input.IsEnabled = _reading is not null;
        _textMode.IsEnabled = _reading is not null;

        if (_text)
        {
            _editorText.Text = Joined(_lines);
            _save.IsEnabled = false;
        }

        Draw();
    }

    /// <summary>Draws both the Vault rules and the Workspace's own.</summary>
    private void Draw()
    {
        DrawVaults();
        DrawRules();

        _count.Text = RuleLines.Count(_lines).ToString(
            System.Globalization.CultureInfo.InvariantCulture
        );

        PaintAdd();
    }

    /// <summary>The heading: what the pane is, how many rules it holds, and how to read it as text.</summary>
    /// <returns>The heading.</returns>
    private Control Heading()
    {
        var label = WorkspaceFurniture.Label("rorolala_vcs.workspace.rules_title");

        var row = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("Auto,Auto,*,Auto"),
            ColumnSpacing = 8,
            VerticalAlignment = VerticalAlignment.Center,
        };

        row.Children.Add(label);
        Grid.SetColumn(_count, 1);
        row.Children.Add(_count);
        Grid.SetColumn(_textMode, 3);
        row.Children.Add(_textMode);

        return row;
    }

    /// <summary>
    /// The section a Vault's rules are drawn in: a heading that folds it and the rows under it.
    /// </summary>
    /// <param name="body">What the section folds.</param>
    /// <returns>The section.</returns>
    private Border Section(StackPanel body)
    {
        var label = WorkspaceFurniture.Label("rorolala_vcs.workspace.vaults_title");
        label[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(PaletteKeys.Muted);

        var lockMark = WorkspaceFurniture.Glyph(
            _host,
            Icons.Lock,
            PaletteKeys.Faint,
            WorkspaceFurniture.GlyphSize
        );
        ToolTip.SetTip(lockMark, RolaI18N.Get("rorolala_vcs.workspace.vaults_read_only_tip"));

        var words = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 6,
            VerticalAlignment = VerticalAlignment.Center,
            Children = { label, _vaultsCount },
        };

        var head = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("Auto,*,Auto"),
            ColumnSpacing = 6,
            VerticalAlignment = VerticalAlignment.Center,
        };
        head.Children.Add(new Border { Width = 12, Child = _vaultsCaret });
        Grid.SetColumn(words, 1);
        head.Children.Add(words);
        Grid.SetColumn(lockMark, 2);
        head.Children.Add(lockMark);

        var fold = new Border
        {
            Height = 32,
            Padding = new Thickness(Edge, 0, Trail, 0),
            Background = Brushes.Transparent,
            Cursor = new Cursor(StandardCursorType.Hand),
            Child = head,
        };
        WorkspaceFurniture.Hover(fold);

        // The heading is read quietly until the pointer finds it, which is how the Overview draws
        // the heading of a group: what folds is the whole row, so the row says it can be pressed.
        fold.PointerEntered += (_, _) =>
            label[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(PaletteKeys.Front);
        fold.PointerExited += (_, _) =>
            label[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(PaletteKeys.Muted);

        fold.PointerPressed += (_, pressed) =>
        {
            if (!pressed.GetCurrentPoint(fold).Properties.IsLeftButtonPressed)
            {
                return;
            }

            _vaultsOpen = !_vaultsOpen;
            WorkspaceFold.Turn(_vaultsCaret, _vaultsOpen);
            WorkspaceFold.Rows(body, _vaultsMotion, !_vaultsOpen);
            pressed.Handled = true;
        };

        var section = new Border
        {
            Child = new StackPanel { Children = { fold, body } },
            BorderThickness = new Thickness(0, 0, 0, 1),
            IsVisible = false,
        };
        section[!Border.BorderBrushProperty] = new DynamicResourceExtension(PaletteKeys.Splitter);

        return section;
    }

    /// <summary>Draws what the Vaults handed down, one block per Vault.</summary>
    private void DrawVaults()
    {
        var vaults = _reading?.Vaults ?? [];

        _vaults.IsVisible = vaults.Count > 0;
        _vaultsCount.Text = vaults.Count.ToString(
            System.Globalization.CultureInfo.InvariantCulture
        );

        _vaultRows.Children.Clear();

        foreach (var vault in vaults)
        {
            var name = new TextBlock
            {
                Text = vault.Name,
                FontFamily = GraphMeasure.Mono,
                FontSize = 12,
                VerticalAlignment = VerticalAlignment.Center,
            };
            name[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(PaletteKeys.Front);

            var note = new TextBlock
            {
                Text = RolaI18N.Get("rorolala_vcs.workspace.vaults_read_only", vault.Patterns.Count),
                FontSize = 11,
                VerticalAlignment = VerticalAlignment.Center,
            };
            note[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(PaletteKeys.Muted);

            var head = new StackPanel
            {
                Orientation = Orientation.Horizontal,
                Spacing = 8,
                Margin = new Thickness(Indent, 4, Trail, 2),
                Children = { name, note },
            };
            _vaultRows.Children.Add(head);

            foreach (var pattern in vault.Patterns)
            {
                _vaultRows.Children.Add(Vaulted(pattern));
            }
        }

        // An open section is as tall as what it holds: a height measured before a fetch added a
        // rule would clip the rule that was fetched.
        if (_vaultsOpen)
        {
            _vaultsBody.MaxHeight = double.PositiveInfinity;
            _vaultsBody.Opacity = 1;
        }
        else
        {
            _vaultsBody.MaxHeight = 0;
            _vaultsBody.Opacity = 0;
        }
    }

    /// <summary>One rule a Vault handed down, which is read and not acted on.</summary>
    /// <param name="pattern">The rule as its file states it.</param>
    /// <returns>The row.</returns>
    private Control Vaulted(string pattern)
    {
        var allow = pattern.StartsWith('!');
        var mark = WorkspaceFurniture.Glyph(
            _host,
            allow ? Icons.CheckCircle : Icons.MinusCircle,
            PaletteKeys.Faint,
            16
        );

        var text = new TextBlock
        {
            Text = allow ? pattern[1..] : pattern,
            FontFamily = GraphMeasure.Mono,
            FontSize = 11.5,
            VerticalAlignment = VerticalAlignment.Center,
            TextTrimming = TextTrimming.CharacterEllipsis,
            TextWrapping = TextWrapping.NoWrap,
        };
        text[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(PaletteKeys.Muted);

        var row = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("16,*"),
            ColumnSpacing = 6,
            Margin = new Thickness(Indent, 0, Trail, 0),
            MinHeight = 26,
            Children = { mark, text },
        };
        Grid.SetColumn(text, 1);

        return row;
    }

    /// <summary>Draws the Workspace's own rules, one row per block.</summary>
    private void DrawRules()
    {
        _rules.Children.Clear();
        _rules.Margin = new Thickness(0, 4, 0, 0);
        _shown = RuleLines.Blocks(_lines);

        if (_reading is null)
        {
            _shown = [];

            if (_read)
            {
                _rules.Children.Add(
                    Line("rorolala_vcs.workspace.rules_unreadable", Edge, 18, Trail, 12)
                );
            }

            return;
        }

        if (_shown.Count == 0)
        {
            _rules.Children.Add(
                Line("rorolala_vcs.workspace.rules_empty", Edge, 18, Trail, 12)
            );
        }

        foreach (var block in _shown)
        {
            _rules.Children.Add(
                block is RulePattern pattern
                    ? PatternRow(pattern)
                    : CommentRow((RuleComment)block)
            );
        }
    }

    /// <summary>One pattern line: what it does, what it matches, and what may be done about it.</summary>
    /// <param name="pattern">The pattern.</param>
    /// <returns>The row.</returns>
    private Control PatternRow(RulePattern pattern)
    {
        var row = new Border
        {
            MinHeight = WorkspaceFurniture.RowHeight,
            Background = Brushes.Transparent,
        };
        WorkspaceFurniture.Hover(row);

        var mark = WorkspaceFurniture.Glyph(
            _host,
            pattern.Allows ? Icons.CheckCircle : Icons.MinusCircle,
            pattern.Allows ? PaletteKeys.Add : PaletteKeys.Faint,
            16
        );
        var text = Pattern(pattern);

        var facts = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("16,*"),
            ColumnSpacing = 6,
            VerticalAlignment = VerticalAlignment.Center,
            Children = { mark, text },
        };
        Grid.SetColumn(text, 1);

        var actions = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 4,
            VerticalAlignment = VerticalAlignment.Center,
        };
        actions.Children.Add(
            WorkspaceFurniture.Square(
                _host,
                pattern.Allows ? Icons.MinusCircle : Icons.CheckCircle,
                pattern.Allows
                    ? "rorolala_vcs.workspace.rule_ignore_tip"
                    : "rorolala_vcs.workspace.rule_allow_tip",
                () => Commit(RuleLines.Toggle(_lines, pattern.At))
            )
        );
        actions.Children.Add(
            WorkspaceFurniture.Square(
                _host,
                Icons.TrashBold,
                "rorolala_vcs.workspace.rule_delete_tip",
                () => Commit(RuleLines.Remove(_lines, pattern.At, pattern.At)),
                danger: true
            )
        );

        var cells = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("12,*,Auto"),
            ColumnSpacing = 6,
            VerticalAlignment = VerticalAlignment.Center,
        };
        cells.Children.Add(Grip((pattern.At, pattern.At, row)));
        Grid.SetColumn(facts, 1);
        cells.Children.Add(facts);
        Grid.SetColumn(actions, 2);
        cells.Children.Add(actions);

        row.Padding = new Thickness(10, 3, Trail, 3);
        row.Child = cells;

        return row;
    }

    /// <summary>What a pattern says, with the mark of an allow rule in the colour of one.</summary>
    /// <param name="pattern">The pattern.</param>
    /// <returns>The line of text.</returns>
    private static TextBlock Pattern(RulePattern pattern)
    {
        var inlines = new InlineCollection();
        var text = new TextBlock
        {
            FontFamily = GraphMeasure.Mono,
            FontSize = 12,
            VerticalAlignment = VerticalAlignment.Center,
            TextTrimming = TextTrimming.CharacterEllipsis,
            TextWrapping = TextWrapping.NoWrap,
            Inlines = inlines,
        };
        text[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(PaletteKeys.Front);

        if (pattern.Allows)
        {
            var bang = new Run("!");
            bang[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(PaletteKeys.Add);
            inlines.Add(bang);
        }

        inlines.Add(new Run(pattern.Pattern));
        ToolTip.SetTip(text, pattern.Text);

        return text;
    }

    /// <summary>One note: its prose, and the two things that may be done with it as a whole.</summary>
    /// <param name="comment">The note.</param>
    /// <returns>The row.</returns>
    private Control CommentRow(RuleComment comment) =>
        _editingComment == comment.Start ? CommentEditor(comment) : CommentRead(comment);

    /// <summary>A note as it is read.</summary>
    /// <param name="comment">The note.</param>
    /// <returns>The row.</returns>
    private Control CommentRead(RuleComment comment)
    {
        var row = new Border
        {
            MinHeight = WorkspaceFurniture.RowHeight,
            Background = Brushes.Transparent,
        };
        WorkspaceFurniture.Hover(row);

        var body = new StackPanel();
        foreach (var line in comment.Lines)
        {
            var text = new TextBlock
            {
                Text = line,
                FontSize = 11.5,
                LineHeight = NoteLine,
                MinHeight = NoteLine,
                TextWrapping = TextWrapping.Wrap,
            };
            text[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(PaletteKeys.Muted);
            body.Children.Add(text);
        }

        var actions = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 4,
            VerticalAlignment = VerticalAlignment.Top,
        };
        actions.Children.Add(
            WorkspaceFurniture.Square(
                _host,
                Icons.Pencil,
                "rorolala_vcs.workspace.comment_edit_tip",
                () =>
                {
                    _editingComment = comment.Start;
                    DrawRules();
                }
            )
        );
        actions.Children.Add(
            WorkspaceFurniture.Square(
                _host,
                Icons.TrashBold,
                "rorolala_vcs.workspace.comment_delete_tip",
                () => Commit(RuleLines.Remove(_lines, comment.Start, comment.End)),
                danger: true
            )
        );

        var mark = Mark();
        var cells = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("12,16,*,Auto"),
            ColumnSpacing = 6,
        };
        cells.Children.Add(Grip((comment.Start, comment.End, row)));
        cells.Children.Add(mark);
        Grid.SetColumn(mark, 1);
        Grid.SetColumn(body, 2);
        cells.Children.Add(body);
        Grid.SetColumn(actions, 3);
        cells.Children.Add(actions);

        row.Padding = new Thickness(10, 7, Trail, 7);
        row.Child = cells;

        return row;
    }

    /// <summary>A note being edited, which is the block's prose rather than the file's marks.</summary>
    /// <param name="comment">The note.</param>
    /// <returns>The row.</returns>
    private Control CommentEditor(RuleComment comment)
    {
        var row = new Border
        {
            MinHeight = WorkspaceFurniture.RowHeight,
            Background = Brushes.Transparent,
        };

        var editor = new TextBox
        {
            Text = string.Join('\n', comment.Lines),
            AcceptsReturn = true,
            TextWrapping = TextWrapping.Wrap,
            MinHeight = 48,
            Padding = new Thickness(8, 6),
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(0),
            FontSize = 12,
            VerticalContentAlignment = VerticalAlignment.Top,
        };
        editor[!TemplatedControl.BackgroundProperty] = new DynamicResourceExtension(PaletteKeys.Dark);
        editor[!TemplatedControl.BorderBrushProperty] = new DynamicResourceExtension(
            PaletteKeys.Splitter
        );

        // The keys are taken on the way down, before the field reads them: Enter is a new line in a
        // note, so the two combinations that finish one are not the field's to take.
        editor.AddHandler(
            InputElement.KeyDownEvent,
            (_, pressed) =>
            {
                if (pressed.Key == Key.Escape)
                {
                    pressed.Handled = true;
                    _editingComment = null;
                    DrawRules();

                    return;
                }

                if (
                    pressed.Key == Key.Enter
                    && (
                        pressed.KeyModifiers.HasFlag(KeyModifiers.Control)
                        || pressed.KeyModifiers.HasFlag(KeyModifiers.Meta)
                    )
                )
                {
                    pressed.Handled = true;
                    Save(comment, editor.Text ?? string.Empty);
                }
            },
            RoutingStrategies.Tunnel
        );

        var actions = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 4,
            VerticalAlignment = VerticalAlignment.Top,
        };
        actions.Children.Add(
            WorkspaceFurniture.Square(
                _host,
                Icons.CheckCircle,
                "rorolala_vcs.workspace.comment_save_tip",
                () => Save(comment, editor.Text ?? string.Empty)
            )
        );
        actions.Children.Add(
            WorkspaceFurniture.Square(
                _host,
                Icons.X,
                "rorolala_vcs.workspace.comment_cancel_tip",
                () =>
                {
                    _editingComment = null;
                    DrawRules();
                }
            )
        );

        var mark = Mark();
        var cells = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("12,16,*,Auto"),
            ColumnSpacing = 6,
        };
        cells.Children.Add(new Border { Width = 12 });
        cells.Children.Add(mark);
        Grid.SetColumn(mark, 1);
        Grid.SetColumn(editor, 2);
        cells.Children.Add(editor);
        Grid.SetColumn(actions, 3);
        cells.Children.Add(actions);

        row.Padding = new Thickness(10, 7, Trail, 7);
        row.Child = cells;

        Dispatcher.UIThread.Post(() =>
        {
            editor.Focus();
            editor.CaretIndex = editor.Text?.Length ?? 0;
        });

        return row;
    }

    /// <summary>Saves the note being edited, or takes it away when it says nothing.</summary>
    /// <param name="comment">The note.</param>
    /// <param name="text">What the editor holds.</param>
    private void Save(RuleComment comment, string text)
    {
        var lines = text.Trim().Length == 0
            ? RuleLines.Remove(_lines, comment.Start, comment.End)
            : RuleLines.Comment(_lines, comment.Start, comment.End, text);

        Commit(lines);
    }

    /// <summary>The <c>#</c> that says a row is a note rather than a pattern.</summary>
    /// <returns>The mark.</returns>
    private static TextBlock Mark()
    {
        var mark = new TextBlock
        {
            Text = "#",
            FontFamily = GraphMeasure.Mono,
            FontSize = 11.5,
            TextAlignment = TextAlignment.Center,
            Margin = new Thickness(0, 1, 0, 0),
        };
        mark[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(PaletteKeys.Faint);

        return mark;
    }

    /// <summary>The grip a block is dragged by, which is drawn whether or not the pointer is on it.</summary>
    /// <param name="block">The block's lines and the row they are drawn in.</param>
    /// <returns>The grip.</returns>
    private Control Grip((int Start, int End, Border Row) block)
    {
        var grip = new Border
        {
            Width = 12,
            Height = 24,
            VerticalAlignment = VerticalAlignment.Top,
            Cursor = new Cursor(StandardCursorType.SizeAll),
            Child = WorkspaceFurniture.Glyph(
                _host,
                Icons.DotsSixVertical,
                PaletteKeys.Faint,
                12
            ),
        };
        ToolTip.SetTip(grip, RolaI18N.Get("rorolala_vcs.workspace.rule_drag_tip"));

        var start = block.Start;
        var end = block.End;
        var row = block.Row;

        grip.PointerPressed += (_, pressed) =>
        {
            if (!pressed.GetCurrentPoint(grip).Properties.IsLeftButtonPressed)
            {
                return;
            }

            _drag = (start, end, row);
            _drop = null;
            row.Opacity = 0.4;
            pressed.Pointer.Capture(grip);
            pressed.Handled = true;
        };

        grip.PointerMoved += (_, moved) =>
        {
            if (_drag is not { } dragged)
            {
                return;
            }

            moved.Handled = true;
            ClearDrop();

            var at = moved.GetPosition(_rules).Y;
            _drop = Landing(at, dragged.Row);

            if (_drop is { } drop)
            {
                drop.Row.BoxShadow = Marker(drop.After);
            }
        };

        grip.PointerReleased += (_, released) =>
        {
            if (_drag is not { } dragged)
            {
                return;
            }

            // The landing is read before the pointer lets the grip go: letting go raises the lost
            // capture, which ends the drag and takes the landing with it.
            var landing = _drop;
            EndDrag();
            released.Pointer.Capture(null);

            if (landing is { } drop)
            {
                Commit(
                    RuleLines.Move(
                        _lines,
                        dragged.Start,
                        dragged.End,
                        drop.Block.Start,
                        drop.Block.End,
                        drop.After
                    )
                );
            }
        };

        grip.PointerCaptureLost += (_, _) =>
        {
            if (_drag is not null)
            {
                EndDrag();
            }
        };

        return grip;
    }

    /// <summary>The row a pointer at one height would land against.</summary>
    /// <remarks>
    /// The dragged row is not a landing, and neither is the gap where it was: a note dropped on
    /// itself is a note that stays where it is. The pointer above every row lands before the first,
    /// below every row lands after the last, and between two rows lands against the one it is inside
    /// of, before or after that row's middle.
    /// </remarks>
    /// <param name="at">The pointer's height in the list.</param>
    /// <param name="dragged">The row being dragged.</param>
    /// <returns>The block and row it would land on, and whether it lands after it.</returns>
    private (RuleBlock Block, Border Row, bool After)? Landing(double at, Border dragged)
    {
        var rows = new List<(RuleBlock Block, Border Row)>();

        for (var index = 0; index < _rules.Children.Count; index++)
        {
            if (_rules.Children[index] is not Border row || ReferenceEquals(row, dragged))
            {
                continue;
            }

            if (BlockAt(index) is { } block)
            {
                rows.Add((block, row));
            }
        }

        if (rows.Count == 0)
        {
            return null;
        }

        if (at < rows[0].Row.Bounds.Top)
        {
            return (rows[0].Block, rows[0].Row, false);
        }

        if (at > rows[^1].Row.Bounds.Bottom)
        {
            return (rows[^1].Block, rows[^1].Row, true);
        }

        foreach (var (block, row) in rows)
        {
            if (at >= row.Bounds.Top && at <= row.Bounds.Bottom)
            {
                return (block, row, at > row.Bounds.Top + (row.Bounds.Height / 2));
            }
        }

        // The pointer between two rows is against the one below it rather than nothing, so that a
        // gap left by the dragged row is still a place a note can be put.
        foreach (var (block, row) in rows)
        {
            if (at < row.Bounds.Top)
            {
                return (block, row, false);
            }
        }

        return (rows[^1].Block, rows[^1].Row, true);
    }

    /// <summary>The block a row of the list draws.</summary>
    /// <param name="index">The row's place in the list.</param>
    /// <returns>The block, or nothing when the row is the line an empty list says.</returns>
    private RuleBlock? BlockAt(int index) => index < _shown.Count ? _shown[index] : null;

    /// <summary>The two pixel line a landing is drawn as, over the row or under it.</summary>
    /// <param name="after">Which side of the row.</param>
    /// <returns>The shadow.</returns>
    private BoxShadows Marker(bool after) =>
        new(
            new BoxShadow
            {
                OffsetX = 0,
                OffsetY = after ? -2 : 2,
                Blur = 0,
                Spread = 0,
                IsInset = true,
                Color = _host.Palette.Primary,
            }
        );

    /// <summary>Draws the block that is no longer being dragged as it was.</summary>
    private void EndDrag()
    {
        if (_drag is { } dragged)
        {
            dragged.Row.Opacity = 1;
        }

        ClearDrop();
        _drag = null;
        _drop = null;
    }

    /// <summary>Takes the landing line off every row.</summary>
    private void ClearDrop()
    {
        foreach (var child in _rules.Children)
        {
            if (child is Border row)
            {
                row.BoxShadow = default;
            }
        }
    }

    /// <summary>
    /// Writes a changed list of lines, then draws what was written.
    /// </summary>
    /// <remarks>
    /// The file is written before the list is drawn, because the file is the truth: a row that showed
    /// a change the write refused would be the pane disagreeing with what the next reading ignores.
    /// A write that failed leaves the list as it was and says why.
    /// <para>
    /// What the rules govern is kept by the listing as well, so it is let go of and every view of the
    /// files is told: a rule turned off is an entry that appears, and one turned on is an entry that
    /// goes, and neither is drawn by reading the rule file alone.
    /// </para>
    /// </remarks>
    /// <param name="lines">The lines to write.</param>
    /// <returns>Whether the file was written.</returns>
    private bool Commit(IReadOnlyList<string> lines)
    {
        if (_reading is null || ReferenceEquals(lines, _lines))
        {
            return false;
        }

        try
        {
            _reading.Write(lines);
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            FailureNotice.Show(
                _host.Dialogs,
                _host.Icons,
                RolaI18N.Get("rorolala_vcs.workspace.rules_write_failed"),
                error.Message
            );

            return false;
        }

        _lines = lines;
        _editingComment = null;

        if (_text)
        {
            _editorText.Text = Joined(lines);
            _save.IsEnabled = false;
        }

        Draw();

        RolaIgnores.Forget();
        _host.Files.Touch();

        return true;
    }

    /// <summary>Opens or closes the file as text.</summary>
    /// <param name="open">Whether the text is to be shown.</param>
    private void SetText(bool open)
    {
        if (_reading is null)
        {
            return;
        }

        _text = open;
        _scroll.IsVisible = !open;
        _adder.IsVisible = !open;
        _editor.IsVisible = open;
        _textMode.IsVisible = !open;

        if (!open)
        {
            return;
        }

        _editingComment = null;
        _drag = null;
        _drop = null;
        _editorText.Text = Joined(_lines);
        _save.IsEnabled = false;

        Dispatcher.UIThread.Post(() =>
        {
            _editorText.Focus();
            _editorText.CaretIndex = 0;
        });
    }

    /// <summary>Writes what the text editor holds.</summary>
    private void SaveText()
    {
        var text = _editorText.Text ?? string.Empty;
        var lines = new List<string>();

        foreach (var line in text.Replace("\r\n", "\n", StringComparison.Ordinal).Split('\n'))
        {
            lines.Add(line.TrimEnd('\r'));
        }

        // A file ends with a newline, and splitting its text gives one empty line for it: that line
        // is the file's ending rather than a rule the reader wrote.
        if (lines.Count > 0 && lines[^1].Length == 0)
        {
            lines.RemoveAt(lines.Count - 1);
        }

        if (Commit(lines))
        {
            SetText(false);
        }
    }

    /// <summary>The text the file holds, as one string for the editor.</summary>
    /// <param name="lines">The file's lines.</param>
    /// <returns>The lines, one per line.</returns>
    private static string Joined(IReadOnlyList<string> lines) => string.Join('\n', lines);

    /// <summary>The strip a pattern or a note is typed into.</summary>
    /// <returns>The strip.</returns>
    private Border Strip()
    {
        var strip = new Border
        {
            Height = Adder,
            BorderThickness = new Thickness(0, 1, 0, 0),
        };
        strip[!Border.BorderBrushProperty] = new DynamicResourceExtension(PaletteKeys.Splitter);

        var cells = new Grid { ColumnDefinitions = new ColumnDefinitions("*,96") };
        cells.Children.Add(_input);
        Grid.SetColumn(_add, 1);
        cells.Children.Add(_add);

        strip.Child = cells;

        return strip;
    }

    /// <summary>The field the strip is rather than a control laid on it.</summary>
    /// <returns>The field.</returns>
    private TextBox Field()
    {
        var field = new TextBox
        {
            PlaceholderText = RolaI18N.Get("rorolala_vcs.workspace.rules_add_hint"),
            AcceptsReturn = false,
            TextWrapping = TextWrapping.NoWrap,
            BorderThickness = new Thickness(0),
            CornerRadius = new CornerRadius(0),
            Background = Brushes.Transparent,
            Padding = new Thickness(Edge, 0),
            FontFamily = GraphMeasure.Mono,
            FontSize = 12.5,
            VerticalContentAlignment = VerticalAlignment.Center,
            Cursor = new Cursor(StandardCursorType.Ibeam),
        };

        // The theme draws a ring and a line on the template's border once the field has the keyboard.
        // The strip is a part of the dock rather than a field on it, so those are given the nothing
        // they are to draw as local values, which the focus rule cannot outrank.
        field.TemplateApplied += (_, applied) =>
        {
            if (applied.NameScope.Find<Border>("border") is { } border)
            {
                border.BoxShadow = default;
                border.BorderThickness = new Thickness(0);
                border.CornerRadius = new CornerRadius(0);
                border.BorderBrush = Brushes.Transparent;
            }
        };

        field.TextChanged += (_, _) => PaintAdd();
        field.KeyDown += (_, pressed) =>
        {
            if (pressed.Key == Key.Enter)
            {
                pressed.Handled = true;
                Add();
            }
        };

        return field;
    }

    /// <summary>The word at the strip's end, which adds what the field holds.</summary>
    /// <returns>The button.</returns>
    private Button Word()
    {
        var button = new Button
        {
            Content = _addWord,
            Background = Brushes.Transparent,
            BorderThickness = new Thickness(1, 0, 0, 0),
            CornerRadius = new CornerRadius(0),
            Padding = new Thickness(Edge, 0),
            Width = AddWidth,
            Height = Adder,
            HorizontalContentAlignment = HorizontalAlignment.Center,
            VerticalContentAlignment = VerticalAlignment.Center,
        };
        button[!TemplatedControl.BorderBrushProperty] = new DynamicResourceExtension(
            PaletteKeys.Splitter
        );
        button.Click += (_, _) => Add();

        return button;
    }

    /// <summary>Adds what the field holds, which is the last line of the file.</summary>
    private void Add()
    {
        var text = _input.Text ?? string.Empty;
        var added = RuleLines.Add(_lines, text);

        if (ReferenceEquals(added, _lines))
        {
            return;
        }

        if (!Commit(added))
        {
            return;
        }

        _input.Text = string.Empty;

        Dispatcher.UIThread.Post(
            () =>
            {
                _scroll.ScrollToEnd();
                _input.Focus();
            },
            DispatcherPriority.Background
        );
    }

    /// <summary>Draws the strip's word as able to be pressed only when there is something to add.</summary>
    private void PaintAdd()
    {
        var held = _input.Text?.Trim() is { Length: > 0 };
        _add.IsEnabled = held && _reading is not null;
        _addWord[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(
            held && _reading is not null ? PaletteKeys.Front : PaletteKeys.Faint
        );
        _add.Cursor = new Cursor(held ? StandardCursorType.Hand : StandardCursorType.Arrow);
    }

    /// <summary>The text the file is edited in.</summary>
    /// <returns>The field.</returns>
    private TextBox Editor()
    {
        var editor = new TextBox
        {
            AcceptsReturn = true,
            TextWrapping = TextWrapping.Wrap,
            Padding = new Thickness(Edge, 10),
            BorderThickness = new Thickness(0, 0, 0, 1),
            CornerRadius = new CornerRadius(0),
            FontFamily = GraphMeasure.Mono,
            FontSize = 11.5,
            VerticalContentAlignment = VerticalAlignment.Top,
        };
        editor[!TemplatedControl.BackgroundProperty] = new DynamicResourceExtension(PaletteKeys.Dark);
        editor[!TemplatedControl.BorderBrushProperty] = new DynamicResourceExtension(
            PaletteKeys.Splitter
        );

        editor.TemplateApplied += (_, applied) =>
        {
            if (applied.NameScope.Find<Border>("border") is { } border)
            {
                border.BoxShadow = default;
                border.BorderThickness = new Thickness(0);
                border.CornerRadius = new CornerRadius(0);
                border.BorderBrush = Brushes.Transparent;
            }
        };

        editor.TextChanged += (_, _) =>
        {
            _save.IsEnabled = _text
                && !string.Equals(
                    editor.Text ?? string.Empty,
                    Joined(_lines),
                    StringComparison.Ordinal
                );
        };

        editor.KeyDown += (_, pressed) =>
        {
            if (pressed.Key == Key.Escape)
            {
                pressed.Handled = true;
                SetText(false);
            }
            else if (
                pressed.Key == Key.Enter
                && (
                    pressed.KeyModifiers.HasFlag(KeyModifiers.Control)
                    || pressed.KeyModifiers.HasFlag(KeyModifiers.Meta)
                )
            )
            {
                pressed.Handled = true;
                SaveText();
            }
        };

        return editor;
    }

    /// <summary>A word that writes, in the look's primary.</summary>
    /// <param name="key">The i18n key of the word.</param>
    /// <param name="press">What pressing it does.</param>
    /// <returns>The button.</returns>
    private static Button Primary(string key, Action press)
    {
        var button = new Button
        {
            Content = RolaI18N.Get(key),
            Height = WorkspaceFurniture.SquareSize,
            Padding = new Thickness(10, 0),
            CornerRadius = new CornerRadius(0),
            Classes = { "primary" },
        };
        button.Click += (_, _) => press();

        return button;
    }

    /// <summary>A word that writes nothing.</summary>
    /// <param name="key">The i18n key of the word.</param>
    /// <param name="press">What pressing it does.</param>
    /// <returns>The button.</returns>
    private static Button Ghost(string key, Action press)
    {
        var button = new Button
        {
            Content = RolaI18N.Get(key),
            Height = WorkspaceFurniture.SquareSize,
            Padding = new Thickness(10, 0),
            CornerRadius = new CornerRadius(0),
            Classes = { "ghost" },
        };
        button.Click += (_, _) => press();

        return button;
    }

    /// <summary>One quiet line where there is nothing else to draw.</summary>
    /// <param name="key">The i18n key of the line.</param>
    /// <param name="left">The room at its left.</param>
    /// <param name="top">The room above it.</param>
    /// <param name="right">The room at its right.</param>
    /// <param name="bottom">The room below it.</param>
    /// <returns>The line.</returns>
    private static TextBlock Line(string key, double left, double top, double right, double bottom)
    {
        var line = new TextBlock
        {
            Text = RolaI18N.Get(key),
            FontSize = 12,
            TextWrapping = TextWrapping.Wrap,
            Margin = new Thickness(left, top, right, bottom),
        };
        line[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(PaletteKeys.Muted);

        return line;
    }
}

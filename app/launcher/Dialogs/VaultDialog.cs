using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Media;
using RorolalaDesktop.I18n;
using RorolalaLauncher.Controls;
using RorolalaLauncher.Rola;
using RorolalaLauncher.Theming;

namespace RorolalaLauncher.Dialogs;

/// <summary>
/// Adds an address the launcher hosts, or edits one it already hosts.
/// </summary>
/// <remarks>
/// Two fields and nothing else: what an address is called and what it is. The format is stated by the
/// address field's placeholder rather than by a line under it, so the form is as short as the two questions
/// in it. What is written is checked here before it is written down, and an address already in the dock,
/// whether it came from the registry or from this file, is refused rather than added twice.
/// </remarks>
internal sealed class VaultDialog : DialogWindow
{
    /// <summary>The colours in force.</summary>
    private readonly Skin _skin;

    /// <summary>The entry being edited, or nothing when one is being added.</summary>
    private readonly VaultEntry? _editing;

    /// <summary>Every address the dock holds, which is what a new one is compared against.</summary>
    private readonly IReadOnlyList<VaultEntry> _all;

    /// <summary>Where the name is written.</summary>
    private readonly TextBox _name;

    /// <summary>Where the address is written.</summary>
    private readonly TextBox _address;

    /// <summary>What is wrong with the name.</summary>
    private readonly TextBlock _nameSays;

    /// <summary>What is wrong with the address.</summary>
    private readonly TextBlock _addressSays;

    /// <summary>Makes the window.</summary>
    /// <param name="skin">The colours in force.</param>
    /// <param name="editing">The entry being edited, or nothing when one is being added.</param>
    /// <param name="all">Every address the dock holds.</param>
    public VaultDialog(Skin skin, VaultEntry? editing, IReadOnlyList<VaultEntry> all)
        : base(
            skin,
            RolaI18N.Get(editing is null ? "launcher.vault.add_title" : "launcher.vault.edit_title"),
            440,
            320
        )
    {
        ArgumentNullException.ThrowIfNull(all);

        _skin = skin;
        _editing = editing;
        _all = all;

        Height = double.NaN;
        SizeToContent = SizeToContent.Height;
        Body.Margin = new Thickness(18, 10, 18, 16);
        Body.Spacing = 13;

        _name = Field(skin, RolaI18N.Get("launcher.vault.name_placeholder"));

        _name.Text = editing?.Name ?? string.Empty;
        _name.Height = 34;
        _name.CornerRadius = new CornerRadius(5);

        _address = Field(skin, RolaI18N.Get("launcher.vault.address_placeholder"));

        _address.Text = editing?.Address ?? string.Empty;
        _address.Height = 34;
        _address.CornerRadius = new CornerRadius(5);

        _nameSays = Says(skin);
        _addressSays = Says(skin);

        Body.Children.Add(Block(skin, RolaI18N.Get("launcher.vault.name"), _name, _nameSays));
        Body.Children.Add(Block(skin, RolaI18N.Get("launcher.vault.address"), _address, _addressSays));

        var cancel = new FlatButton(Line(skin, RolaI18N.Get("launcher.common.cancel")), skin)
        {
            Height = 32,
            CornerRadius = new CornerRadius(5),
        };

        var submit = new FlatButton(
            Line(skin, RolaI18N.Get(editing is null ? "launcher.vault.add" : "launcher.vault.save")),
            skin,
            ButtonKind.Primary
        )
        {
            Height = 32,
            CornerRadius = new CornerRadius(5),
        };

        cancel.Clicked += (_, _) => Close();
        submit.Clicked += (_, _) => Commit();

        _name.TextChanged += (_, _) => Clear(_nameSays);
        _address.TextChanged += (_, _) => Clear(_addressSays);
        _name.KeyDown += (_, e) => SubmitOnEnter(e);
        _address.KeyDown += (_, e) => SubmitOnEnter(e);

        Body.Children.Add(Row(cancel, submit));

        Opened += (_, _) => _name.Focus();
    }

    /// <summary>Whether the window wrote something, which is what the caller saves.</summary>
    public bool Saved { get; private set; }

    /// <summary>The name as it was saved.</summary>
    public string SavedName { get; private set; } = string.Empty;

    /// <summary>The address as it was saved.</summary>
    public string SavedAddress { get; private set; } = string.Empty;

    /// <summary>The name as it is written.</summary>
    private string WrittenName => _name.Text?.Trim() ?? string.Empty;

    /// <summary>The address as it is written.</summary>
    private string WrittenAddress => _address.Text?.Trim() ?? string.Empty;

    /// <summary>A field's name and field, with the line that says what is wrong under it.</summary>
    /// <param name="skin">The colours in force.</param>
    /// <param name="label">What the field is called.</param>
    /// <param name="field">The field.</param>
    /// <param name="says">Where what is wrong is written.</param>
    /// <returns>The block.</returns>
    private static Control Block(Skin skin, string label, TextBox field, TextBlock says)
    {
        var block = new StackPanel { Spacing = 5 };

        block.Children.Add(Line(skin, label, 11.5, muted: true));
        block.Children.Add(field);
        block.Children.Add(says);

        return block;
    }

    /// <summary>A line that says what is wrong with a field, hidden until something is.</summary>
    /// <param name="skin">The colours in force.</param>
    /// <returns>The line.</returns>
    private static TextBlock Says(Skin skin)
    {
        var line = Line(skin, string.Empty, 11.5, muted: true);

        line.IsVisible = false;
        line.TextWrapping = TextWrapping.NoWrap;
        line.TextTrimming = TextTrimming.CharacterEllipsis;

        return line;
    }

    /// <summary>Reads the two fields and, when they are both right, saves what they say.</summary>
    private void Commit()
    {
        Clear(_nameSays);
        Clear(_addressSays);

        var name = WrittenName;
        var address = WrittenAddress;
        var problem = false;

        if (name.Length == 0)
        {
            Say(_nameSays, RolaI18N.Get("launcher.vault.name_empty"));
            _name.BorderBrush = _skin.Alarm;
            problem = true;
        }

        if (!VaultAddressSyntax.Link(address))
        {
            Say(_addressSays, RolaI18N.Get("launcher.vault.address_invalid"));
            _address.BorderBrush = _skin.Alarm;
            problem = true;
        }
        else if (_all.Any(entry =>
                !ReferenceEquals(entry, _editing)
                && string.Equals(entry.Address, address, StringComparison.OrdinalIgnoreCase)))
        {
            Say(_addressSays, RolaI18N.Get("launcher.vault.address_duplicate"));
            _address.BorderBrush = _skin.Alarm;
            problem = true;
        }

        if (problem)
        {
            return;
        }

        Saved = true;
        SavedName = name;
        SavedAddress = address;

        Close();
    }

    /// <summary>Submits the form when Enter is pressed in either field.</summary>
    /// <param name="e">The key event.</param>
    private void SubmitOnEnter(KeyEventArgs e)
    {
        ArgumentNullException.ThrowIfNull(e);

        if (e.Key != Key.Enter)
        {
            return;
        }

        e.Handled = true;
        Commit();
    }

    /// <summary>Puts a line under the field it is about, or takes the line away.</summary>
    /// <param name="line">The line.</param>
    /// <param name="says">What it says.</param>
    private void Say(TextBlock line, string says)
    {
        line.Text = says;
        line.Foreground = _skin.Alarm;
        line.IsVisible = true;
    }

    /// <summary>Takes a line away and puts the field it is about back the way it was.</summary>
    /// <param name="line">The line.</param>
    private void Clear(TextBlock line)
    {
        line.Text = string.Empty;
        line.IsVisible = false;

        if (ReferenceEquals(line, _nameSays))
        {
            _name.BorderBrush = _skin.Splitter;
        }
        else
        {
            _address.BorderBrush = _skin.Splitter;
        }
    }
}

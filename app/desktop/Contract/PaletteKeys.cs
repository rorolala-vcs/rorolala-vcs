namespace RorolalaDesktop.Contract;

/// <summary>
/// The names the look publishes its own colours under, for whatever draws itself.
/// </summary>
/// <remarks>
/// A plugin may not reference the host, so the look's names used to be written as literals in each
/// plugin that drew with them — three plants saying the same word, which disagreed the day one of them
/// was corrected. They are stated here once instead, beside <see cref="IPalette"/>, which is what a
/// plugin reads a colour from when it wants the value rather than a resource to name: the five members
/// of that interface are roles, and these are the same roles as the look publishes them.
/// <para>
/// A plugin reads a name only where the colour has to follow the variant it is drawn in, since a
/// resource named in a <c>DynamicResource</c> is resolved by whatever scope the control ends up in and
/// is answered again when the look replaces the resource in place. A colour it needs as a value is
/// asked for through <see cref="IPluginHost.Palette"/>.
/// </para>
/// </remarks>
public static class PaletteKeys
{
    /// <summary>The five members of <see cref="IPalette"/>, as the names they are published under.</summary>
    /// <remarks>
    /// The roles rather than the colours: which of the look's two sets a name is answered from is the
    /// variant the control stands in, which is what keeps a plugin from having to know which one that is.
    /// </remarks>
    public const string Bright = "rorolala.bg.elevated";

    /// <summary>
    /// The front ink, which is what a mark that is nobody's button is drawn in.
    /// </summary>
    /// <remarks>
    /// Named once more under the role it is read as, because where a mark is not the picture of a
    /// control it is drawn from the look rather than from whatever holds it — and a reader looking for
    /// that answer looks for it under the name the mark is, not under the name the surface is.
    /// </remarks>
    public const string PlainInk = Front;

    /// <inheritdoc cref="Bright" />
    public const string Mid = "rorolala.bg";

    /// <inheritdoc cref="Bright" />
    public const string Dark = "rorolala.bg.sunken";

    /// <inheritdoc cref="Bright" />
    public const string Front = "rorolala.fg";

    /// <inheritdoc cref="Bright" />
    public const string Splitter = "rorolala.border";

    /// <summary>What is written where the front ink would be too loud: a second and a third step of it.</summary>
    public const string Muted = "rorolala.fg.muted";

    /// <inheritdoc cref="Muted" />
    public const string Faint = "rorolala.fg.faint";

    /// <summary>The line a surface is told apart by where a hairline is not enough.</summary>
    public const string Strong = "rorolala.border.strong";

    /// <summary>What is chosen: the primary, and the wash of it a chosen row is filled with.</summary>
    public const string Primary = "rorolala.primary";

    /// <inheritdoc cref="Primary" />
    public const string Selection = "rorolala.selection";

    /// <summary>What is written on the primary, which only a mark drawn on it needs to name.</summary>
    public const string PrimaryText = "rorolala.primary.text";

    /// <summary>The look's shadow, which a raised surface wears.</summary>
    public const string Shadow = "rorolala.shadow";

    /// <summary>The one that is neither: the second colour, spent on what must not read as a choice.</summary>
    public const string Accent = "rorolala.accent";

    /// <summary>The accent laid thin, which is what a marked surface is washed with.</summary>
    public const string AccentWash = "rorolala.accent.wash";

    /// <summary>What was added, and what stands behind it.</summary>
    public const string Add = "rorolala.add";

    /// <inheritdoc cref="Add" />
    public const string AddBackground = "rorolala.add.bg";

    /// <summary>What was removed or failed, and what stands behind it.</summary>
    public const string Del = "rorolala.del";

    /// <inheritdoc cref="Del" />
    public const string DelBackground = "rorolala.del.bg";

    /// <summary>The failure's red laid thin, for a surface that has to be marked rather than filled.</summary>
    public const string DelWash = "rorolala.del.wash";

    /// <summary>What asks for attention, and the wash it stands on.</summary>
    public const string Warn = "rorolala.warn";

    /// <inheritdoc cref="Warn" />
    public const string WarnBackground = "rorolala.warn.bg";
}

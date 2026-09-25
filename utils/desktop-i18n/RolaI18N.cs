namespace RorolalaDesktop.I18n;

/// <summary>
/// The written forms the Desktop program speaks in, read from the files the command line keeps.
/// </summary>
/// <remarks>
/// The files are the ones <c>rust-i18n</c> reads, and they are read the same way: YAML, where each
/// node is a key and each leaf is that key written in every language it has been written in. A form
/// leaves places for values as <c>%{name}</c>, and the values go in by order rather than by name —
/// see <see cref="Get(string, object?)"/>.
/// <para>
/// Several directories may be read together, which is how the host and each of its plugins state
/// what they say in one place: the host registers its directory first and each plugin registers its
/// own during initialization, and a key already supplied is not replaced by a later directory.
/// <see cref="SetTranslationDirectory"/> names one directory on its own, and
/// <see cref="RegisterTranslationDirectory"/> adds one after those already named.
/// </para>
/// <para>
/// Where the files are read from and which language is spoken are both set before anything asks for
/// a form: <see cref="SetTranslationDirectory"/> and <see cref="SetLocale"/>. What they were set to
/// is readable back, so a program that has been handed both — the way the Desktop program is handed
/// the language the command line chose — can say what it is speaking.
/// </para>
/// <para>
/// A key that is in no file, and a key written in no language the program speaks, both read as the
/// key itself. What is wrong is then on the screen rather than hidden behind a blank, and finding
/// it is reading the key and looking for it.
/// </para>
/// </remarks>
public static class RolaI18N
{
    /// <summary>The language spoken when the program has been asked for none.</summary>
    private const string Fallback = "en";

    /// <summary>The directories the translation files are read from, in registration order.</summary>
    private static readonly List<string> Directories = [];

    /// <summary>The language the program speaks in, as the files name it — <c>zh-CN</c>.</summary>
    public static string? Locale { get; private set; }

    /// <summary>Guards what is read, so two threads setting it up do not read it twice.</summary>
    private static readonly object Gate = new();

    /// <summary>Every node the files state, once they have been read.</summary>
    private static Dictionary<string, Dictionary<string, string>>? _nodes;

    /// <summary>Which directories the nodes were read from, so a change is noticed.</summary>
    private static string? _readFrom;

    /// <summary>
    /// The directory the translation files are first read from, or nothing when none is registered.
    /// </summary>
    public static string? TranslationDirectory => Directories.Count > 0 ? Directories[0] : null;

    /// <summary>
    /// Every directory the translation files are read from, in the order they were registered.
    /// </summary>
    public static IReadOnlyList<string> TranslationDirectories => Directories;

    /// <summary>
    /// Names the directory the translation files are read from.
    /// </summary>
    /// <remarks>
    /// The files are not read here: they are read the first time a form is asked for, and again
    /// whenever this names a directory other than the one they were read from. A program that
    /// knows where its translations are before it starts — which is what looking beside itself is
    /// — can say so before anything is drawn.
    /// </remarks>
    /// <param name="directory">The directory every translation file sits under, however deep.</param>
    /// <exception cref="ArgumentException"><paramref name="directory"/> is empty.</exception>
    public static void SetTranslationDirectory(string directory)
    {
        ArgumentException.ThrowIfNullOrEmpty(directory);

        lock (Gate)
        {
            Directories.Clear();
            Directories.Add(directory);
            _nodes = null;
            _readFrom = null;
        }
    }

    /// <summary>
    /// Adds a directory the translation files are read from, after the ones already registered.
    /// </summary>
    /// <remarks>
    /// The directories are read in registration order and a key an earlier directory states is not
    /// replaced by a later one — first registration wins. The host registers its own directory
    /// before the plugins register theirs, so a plugin cannot overwrite what the host says, and a
    /// plugin loaded first is not overwritten by one loaded after it.
    /// <para>
    /// A directory registered twice is one directory: what it states is already in.
    /// </para>
    /// </remarks>
    /// <param name="directory">A directory every translation file sits under, however deep.</param>
    /// <exception cref="ArgumentException"><paramref name="directory"/> is empty.</exception>
    public static void RegisterTranslationDirectory(string directory)
    {
        ArgumentException.ThrowIfNullOrEmpty(directory);

        lock (Gate)
        {
            if (Directories.Contains(directory, StringComparer.Ordinal))
            {
                return;
            }

            Directories.Add(directory);
            _nodes = null;
            _readFrom = null;
        }
    }

    /// <summary>
    /// Names the language the program speaks in.
    /// </summary>
    /// <remarks>
    /// The name is taken as a locale, so <c>zh-CN</c>, <c>zh_CN</c> and <c>zh_CN.UTF-8</c> are the
    /// same language and the last two are read as the first. A language no form has been written
    /// in is spoken by <c>en</c>, which is the language the files fall back to.
    /// </remarks>
    /// <param name="locale">The language to speak, as a locale the files name.</param>
    /// <exception cref="ArgumentException"><paramref name="locale"/> is empty.</exception>
    public static void SetLocale(string locale)
    {
        ArgumentException.ThrowIfNullOrEmpty(locale);

        Locale = AsLocale(locale);
    }

    /// <summary>
    /// The form a node is written in, in the language the program speaks.
    /// </summary>
    /// <param name="node">The key of the node, as the files nest it — <c>pack.help</c>.</param>
    /// <returns>
    /// What the node says, or <paramref name="node"/> itself when the files state no such node, or
    /// state one in no language the program has.
    /// </returns>
    /// <exception cref="InvalidOperationException">No translation directory has been named.</exception>
    /// <exception cref="DirectoryNotFoundException">The directory that was named is not there.</exception>
    /// <exception cref="FormatException">A file under it is not readable as YAML.</exception>
    public static string Get(string node) => Resolve(node, []);

    /// <summary>
    /// The form a node is written in, with one value put in the first place it leaves.
    /// </summary>
    /// <remarks>
    /// Values are matched to places by order, not by name: the first value goes in the first place
    /// the form leaves, the second in the second, and what the places are called is only there for
    /// whoever reads the file. A place with no value left for it is left as it was written, and a
    /// value with no place left for it is dropped.
    /// </remarks>
    /// <param name="node">The key of the node, as the files nest it — <c>pack.help</c>.</param>
    /// <param name="first">The value for the first place the form leaves.</param>
    /// <returns>
    /// What the node says with the values in it, or <paramref name="node"/> itself when the files
    /// state no such node, or state one in no language the program has.
    /// </returns>
    /// <inheritdoc cref="Get(string)"/>
    public static string Get(string node, object? first) => Resolve(node, [first]);

    /// <inheritdoc cref="Get(string, object?)"/>
    public static string Get(string node, object? first, object? second) =>
        Resolve(node, [first, second]);

    /// <inheritdoc cref="Get(string, object?)"/>
    public static string Get(string node, object? first, object? second, object? third) =>
        Resolve(node, [first, second, third]);

    /// <inheritdoc cref="Get(string, object?)"/>
    public static string Get(
        string node,
        object? first,
        object? second,
        object? third,
        object? fourth
    ) => Resolve(node, [first, second, third, fourth]);

    /// <inheritdoc cref="Get(string, object?)"/>
    public static string Get(
        string node,
        object? first,
        object? second,
        object? third,
        object? fourth,
        object? fifth
    ) => Resolve(node, [first, second, third, fourth, fifth]);

    /// <inheritdoc cref="Get(string, object?)"/>
    public static string Get(
        string node,
        object? first,
        object? second,
        object? third,
        object? fourth,
        object? fifth,
        object? sixth
    ) => Resolve(node, [first, second, third, fourth, fifth, sixth]);

    /// <inheritdoc cref="Get(string, object?)"/>
    public static string Get(
        string node,
        object? first,
        object? second,
        object? third,
        object? fourth,
        object? fifth,
        object? sixth,
        object? seventh
    ) => Resolve(node, [first, second, third, fourth, fifth, sixth, seventh]);

    /// <inheritdoc cref="Get(string, object?)"/>
    public static string Get(
        string node,
        object? first,
        object? second,
        object? third,
        object? fourth,
        object? fifth,
        object? sixth,
        object? seventh,
        object? eighth
    ) => Resolve(node, [first, second, third, fourth, fifth, sixth, seventh, eighth]);

    /// <inheritdoc cref="Get(string, object?)"/>
    public static string Get(
        string node,
        object? first,
        object? second,
        object? third,
        object? fourth,
        object? fifth,
        object? sixth,
        object? seventh,
        object? eighth,
        object? ninth
    ) => Resolve(node, [first, second, third, fourth, fifth, sixth, seventh, eighth, ninth]);

    /// <inheritdoc cref="Get(string, object?)"/>
    public static string Get(
        string node,
        object? first,
        object? second,
        object? third,
        object? fourth,
        object? fifth,
        object? sixth,
        object? seventh,
        object? eighth,
        object? ninth,
        object? tenth
    ) =>
        Resolve(node, [first, second, third, fourth, fifth, sixth, seventh, eighth, ninth, tenth]);

    /// <inheritdoc cref="Get(string, object?)"/>
    public static string Get(
        string node,
        object? first,
        object? second,
        object? third,
        object? fourth,
        object? fifth,
        object? sixth,
        object? seventh,
        object? eighth,
        object? ninth,
        object? tenth,
        object? eleventh
    ) =>
        Resolve(
            node,
            [first, second, third, fourth, fifth, sixth, seventh, eighth, ninth, tenth, eleventh]
        );

    /// <inheritdoc cref="Get(string, object?)"/>
    public static string Get(
        string node,
        object? first,
        object? second,
        object? third,
        object? fourth,
        object? fifth,
        object? sixth,
        object? seventh,
        object? eighth,
        object? ninth,
        object? tenth,
        object? eleventh,
        object? twelfth
    ) =>
        Resolve(
            node,
            [
                first,
                second,
                third,
                fourth,
                fifth,
                sixth,
                seventh,
                eighth,
                ninth,
                tenth,
                eleventh,
                twelfth,
            ]
        );

    /// <summary>The form a node is written in, with the values put into its places.</summary>
    private static string Resolve(string node, object?[] values)
    {
        ArgumentException.ThrowIfNullOrEmpty(node);

        if (!Nodes().TryGetValue(node, out var forms))
        {
            return node;
        }

        var form = Form(forms);

        return form is null ? node : Placeholders.Fill(form, values);
    }

    /// <summary>
    /// The form a node is written in, in the language the program speaks, or nothing when it has
    /// been written in neither that language nor the one the files fall back to.
    /// </summary>
    private static string? Form(Dictionary<string, string> forms)
    {
        if (Locale is { } locale && forms.TryGetValue(locale, out var spoken))
        {
            return spoken;
        }

        return forms.TryGetValue(Fallback, out var fallback) ? fallback : null;
    }

    /// <summary>
    /// Every node the files state, reading them the first time and again after a change of
    /// directory.
    /// </summary>
    private static Dictionary<string, Dictionary<string, string>> Nodes()
    {
        lock (Gate)
        {
            if (Directories.Count == 0)
            {
                throw new InvalidOperationException(
                    "no translation directory has been named; call SetTranslationDirectory first"
                );
            }

            var readFrom = string.Join('\n', Directories);

            if (_nodes is not null && _readFrom == readFrom)
            {
                return _nodes;
            }

            var nodes = new Dictionary<string, Dictionary<string, string>>(StringComparer.Ordinal);

            foreach (var directory in Directories)
            {
                if (!Directory.Exists(directory))
                {
                    throw new DirectoryNotFoundException(
                        $"the translation directory {directory} is not there"
                    );
                }

                // First registration wins: what an earlier directory states is kept as it is. Within
                // one directory a later file still replaces an earlier one, since the files there are
                // one set and their read order is by path.
                foreach (var (key, forms) in TranslationFiles.Read(directory))
                {
                    nodes.TryAdd(key, forms);
                }
            }

            _nodes = nodes;
            _readFrom = readFrom;

            return _nodes;
        }
    }

    /// <summary>
    /// A language as the files name it, so that what the machine calls a locale is one.
    /// </summary>
    /// <remarks>
    /// A locale on a machine may carry the encoding it is written in — <c>zh_CN.UTF-8</c> — and
    /// separates the region with an underscore where the files use a hyphen, so neither is left in
    /// the name this speaks by.
    /// </remarks>
    private static string AsLocale(string locale)
    {
        var language = locale.Split('.', 2)[0];

        return language.Replace('_', '-');
    }
}

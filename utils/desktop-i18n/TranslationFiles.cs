using YamlDotNet.RepresentationModel;

namespace RorolalaDesktop.I18n;

/// <summary>
/// Reads a directory of translation files into the nodes they state.
/// </summary>
/// <remarks>
/// The files are the ones <c>rust-i18n</c> reads for the command line, and they are read the way
/// it reads them: every <c>*.yml</c> under the directory is read, however deep it sits, and where
/// a file sits says nothing about what it holds — the files are gathered into one set of nodes, so
/// a key belongs to the file that states it rather than to the directory it is filed under.
/// <para>
/// A node is a mapping of locale to written form, which is a mapping whose every value is written
/// out. Everything else is a step on the way to one: a mapping with a mapping under it is walked
/// into, and a value that is not a mapping — <c>_version</c>, which every file opens with — is
/// left where it is.
/// </para>
/// </remarks>
internal static class TranslationFiles
{
    /// <summary>The key a file states its format under, which is not a node.</summary>
    private const string ReservedVersionKey = "_version";

    /// <summary>
    /// Every node the translation files under <paramref name="directory"/> state, by its key.
    /// </summary>
    /// <remarks>
    /// A later file stating a key an earlier one did stated replaces it, since a key names one
    /// node and the files are read in one order: by path, so which file wins does not depend on
    /// the order the file system hands them over in.
    /// </remarks>
    /// <exception cref="FormatException">A file is not readable as YAML.</exception>
    public static Dictionary<string, Dictionary<string, string>> Read(string directory)
    {
        var nodes = new Dictionary<string, Dictionary<string, string>>(StringComparer.Ordinal);
        var path = new List<string>();

        foreach (var file in EnumerateFiles(directory))
        {
            var stream = new YamlStream();

            using (var reader = File.OpenText(file))
            {
                try
                {
                    stream.Load(reader);
                }
                catch (YamlDotNet.Core.YamlException error)
                {
                    throw new FormatException($"{file} is not readable as YAML", error);
                }
            }

            foreach (var document in stream.Documents)
            {
                path.Clear();
                Walk(document.RootNode, path, nodes);
            }
        }

        return nodes;
    }

    /// <summary>
    /// The translation files under <paramref name="directory"/>, in one order.
    /// </summary>
    /// <remarks>
    /// The names are not given to the file system to match, since what a name is taken to end with
    /// is not the same on every platform; what a file is, is read off it instead.
    /// </remarks>
    private static IEnumerable<string> EnumerateFiles(string directory) =>
        Directory
            .EnumerateFiles(directory, "*", SearchOption.AllDirectories)
            .Where(IsTranslationFile)
            .OrderBy(file => file, StringComparer.Ordinal);

    /// <summary>Whether a file is one a translation is written in.</summary>
    private static bool IsTranslationFile(string file)
    {
        var extension = Path.GetExtension(file);

        return extension.Equals(".yml", StringComparison.OrdinalIgnoreCase)
            || extension.Equals(".yaml", StringComparison.OrdinalIgnoreCase);
    }

    /// <summary>
    /// Gathers the nodes under one place in a file.
    /// </summary>
    /// <param name="place">Where in the file this is being read.</param>
    /// <param name="path">The key of the place, as it is nested so far.</param>
    /// <param name="nodes">What has been gathered, added to as nodes are found.</param>
    private static void Walk(
        YamlNode place,
        List<string> path,
        Dictionary<string, Dictionary<string, string>> nodes
    )
    {
        if (place is not YamlMappingNode mapping)
        {
            return;
        }

        // A file's own root is not a node: it holds the files' nodes, and the key it would be
        // gathered under is not one anything is reached by.
        if (path.Count > 0 && IsWrittenForm(mapping))
        {
            nodes[string.Join('.', path)] = WrittenForms(mapping);
            return;
        }

        foreach (var (key, under) in mapping.Children)
        {
            if (
                key is not YamlScalarNode { Value: { } name }
                || name == ReservedVersionKey
                || under is not YamlMappingNode
            )
            {
                continue;
            }

            path.Add(name);
            Walk(under, path, nodes);
            path.RemoveAt(path.Count - 1);
        }
    }

    /// <summary>
    /// Whether a mapping is a node: one whose every value is written out, so that what it holds is
    /// the forms of one key rather than the keys under another.
    /// </summary>
    private static bool IsWrittenForm(YamlMappingNode mapping) =>
        mapping.Children.Count > 0
        && mapping.Children.Values.All(value => value is YamlScalarNode);

    /// <summary>The forms a node is written in, by locale.</summary>
    /// <remarks>
    /// A form is stored as it would be read, which is trimmed: a form written as a block scalar
    /// ends with a newline that belongs to the file rather than to what it says, and the command
    /// line trims every form it speaks for the same reason, so the two say the same thing.
    /// </remarks>
    private static Dictionary<string, string> WrittenForms(YamlMappingNode mapping)
    {
        var forms = new Dictionary<string, string>(StringComparer.Ordinal);

        foreach (var (key, value) in mapping.Children)
        {
            if (key is YamlScalarNode { Value: { } locale } && value is YamlScalarNode { Value: { } form })
            {
                forms[locale] = form.Trim();
            }
        }

        return forms;
    }
}

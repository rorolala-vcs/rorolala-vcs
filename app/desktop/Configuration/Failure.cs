namespace RorolalaDesktop.Configuration;

/// <summary>
/// The codes the program exits with when it refuses to start.
/// </summary>
internal enum ExitCode
{
    /// <summary>Clean exit.</summary>
    Clean = 0,

    /// <summary><c>plugins.json</c> failed to load or validate.</summary>
    Plugins = 1,

    /// <summary><c>preference.json</c> failed to load or validate.</summary>
    Preference = 2,

    /// <summary><c>preference.json</c> names a theme or plugin that is not available.</summary>
    Unavailable = 3,
}

/// <summary>
/// A failure that stops the program before a window exists.
/// </summary>
/// <remarks>
/// The reason is written to standard error and the process exits with <see cref="Code"/>. The
/// program does not fall back to a default configuration: a silently ignored configuration error is
/// worse than a loud stop, and both files are plain JSON a person can edit. The natural place to
/// repair plugin configuration — the plugin manager — is inside the program that refuses to start,
/// so recovery is by editing the file, guided by the reason on standard error.
/// </remarks>
internal sealed class ConfigurationFailure : Exception
{
    /// <summary>Makes a failure that carries a code and a reason.</summary>
    /// <param name="code">The code the process exits with.</param>
    /// <param name="reason">The human-readable reason, written to standard error.</param>
    public ConfigurationFailure(ExitCode code, string reason)
        : base(reason) => Code = code;

    /// <summary>The code the process exits with.</summary>
    public ExitCode Code { get; }
}

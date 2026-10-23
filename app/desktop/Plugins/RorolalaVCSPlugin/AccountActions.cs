using System.Diagnostics;
using System.Text;
using System.Text.Json;

namespace RorolalaVCSPlugin;

/// <summary>One account, as `rola account` names it and says whether it is the one in use.</summary>
/// <param name="Name">The name the account is known by.</param>
/// <param name="Current">Whether it is the account the work acts as.</param>
internal sealed record Account(string Name, bool Current);

/// <summary>
/// What the accounts dock does, by running the command line that does it.
/// </summary>
/// <remarks>
/// An account is a private key and the choice of which one the work acts as is Rorolala's, kept in its own
/// file for the user; listing the accounts and naming one are the command line's `rola account`, and making a
/// pair is `rola key generate`. Doing any of it here would be a second answer to the same question, kept in
/// step by hand, so this owns nothing but which words are put to the user.
/// <para>
/// The two that have no command are the dock's own file work: exporting a public key, which is copying the
/// `.pub` beside the account, and removing one, which is taking that pair away. What they act on is found the
/// way the command line finds it — through `rola key`, which lists the keys in the order the scopes are read —
/// so a name resolves to the same file here as it does there.
/// </para>
/// <para>
/// The program is the one this window was opened by, named by the environment the run handed over, because the
/// two are exported together: a `rola` found on the `PATH` could be another one entirely, of another version,
/// against another Vault.
/// </para>
/// </remarks>
internal static class AccountActions
{
    /// <summary>The theme the output is asked for, which is the one this window reads.</summary>
    private const string Theme = "desktop-friendly";

    /// <summary>The environment variable naming the program the window was opened by.</summary>
    private const string Program = "ROLA_EXE";

    /// <summary>The flag that has the command answer in JSON, which is what a listing is read from.</summary>
    private const string Json = "--json";

    /// <summary>
    /// The names that cannot be an account, because `rola account` reads them as its own subcommands.
    /// </summary>
    /// <remarks>
    /// A name is given to `rola account` as a word, and a word it knows as a subcommand is that subcommand
    /// rather than an account: an account called `ls` would list the accounts instead of being chosen, and one
    /// called `current` would print the account in use. Refused here so that a reader is told before a pair is
    /// made under a name that could never be selected.
    /// </remarks>
    public static readonly IReadOnlyList<string> Forbidden = ["current", "ls"];

    /// <summary>Lists the accounts, as `rola account` lists them.</summary>
    /// <param name="directory">Where the run is made, which is the scopes a key is looked for in.</param>
    /// <returns>Whether it ended well, the accounts, and what to say when it did not.</returns>
    public static (bool Done, IReadOnlyList<Account> Accounts, string Said) List(string directory)
    {
        var (done, output, said) = Run([Json, "account"], directory);

        return done ? (true, Listed(output), string.Empty) : (false, [], said);
    }

    /// <summary>Names the account the work acts as, as `rola account &lt;NAME&gt;` records it.</summary>
    /// <param name="name">The account to act as.</param>
    /// <param name="directory">Where the run is made.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    public static (bool Done, string Said) Use(string name, string directory) =>
        Ended(Run(["account", name], directory));

    /// <summary>Makes a key pair for an account, as `rola key generate --install` does.</summary>
    /// <param name="name">What the pair is known by.</param>
    /// <param name="directory">Where the run is made.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    public static (bool Done, string Said) Create(string name, string directory) =>
        Ended(Run(["key", "generate", name, "--install"], directory));

    /// <summary>
    /// The key files for the accounts, in the order the command line ranks the scopes.
    /// </summary>
    /// <remarks>
    /// With `secret`, the private keys are listed — the accounts themselves — and without it the public ones,
    /// which is what a `.pub` beside an account is. The paths are the command line's own answer, so what is
    /// found here is what would be found by a run of `rola key`.
    /// </remarks>
    /// <param name="secret">Whether the private keys are listed rather than the public ones.</param>
    /// <param name="directory">Where the run is made.</param>
    /// <returns>Whether it ended well, the paths, and what to say when it did not.</returns>
    public static (bool Done, IReadOnlyList<string> Paths, string Said) Paths(bool secret, string directory)
    {
        var arguments = new List<string> { Json, "key" };

        if (secret)
        {
            arguments.Add("--pem");
        }

        var (done, output, said) = Run(arguments, directory);

        return done ? (true, PathsOf(output), string.Empty) : (false, [], said);
    }

    /// <summary>
    /// The account a path stands for, which is its stem: `alice.pem` and `alice.pub` are one pair.
    /// </summary>
    /// <param name="path">The key file's path.</param>
    /// <returns>The account's name, or nothing when the path names no file stem.</returns>
    public static string? Named(string path)
    {
        // A path that ends at a separator names a directory rather than a file, so there is no stem to
        // read off it.
        if (path.Length == 0 || path[^1] is '\\' or '/')
        {
            return null;
        }

        // Both separators, for the same reason a name is read off a path at all: it is a path a person
        // chose, and on Windows either separator may have been used.
        var index = path.LastIndexOfAny(['\\', '/']);
        var name = index >= 0 ? path[(index + 1)..] : path;
        var stem = Path.GetFileNameWithoutExtension(name);

        return stem is { Length: > 0 } ? stem : null;
    }

    /// <summary>What `rola account` printed, read into the accounts it named.</summary>
    /// <remarks>
    /// Read from the result rather than from the work, because the accounts are Rorolala's own answer wherever
    /// a key sits: a search here would be a second answer to keep in step with the command's. An answer that
    /// cannot be read is answered as no accounts, and what the command said is what is shown.
    /// </remarks>
    /// <param name="output">What the command printed, which is one JSON object when it was asked for one.</param>
    /// <returns>The accounts, which is none when the answer does not read.</returns>
    internal static IReadOnlyList<Account> Listed(string output)
    {
        ArgumentNullException.ThrowIfNull(output);

        try
        {
            using var document = JsonDocument.Parse(output);

            if (
                document.RootElement.ValueKind is not JsonValueKind.Object
                || !document.RootElement.TryGetProperty("names", out var names)
                || names.ValueKind is not JsonValueKind.Array
            )
            {
                return [];
            }

            var current =
                document.RootElement.TryGetProperty("current", out var held)
                && held.ValueKind is JsonValueKind.String
                    ? held.GetString()
                    : null;

            return
            [
                .. names
                    .EnumerateArray()
                    .Select(name => name.GetString())
                    .Where(name => name is { Length: > 0 })
                    .Select(name => new Account(name!, string.Equals(name, current, StringComparison.Ordinal))),
            ];
        }
        catch (JsonException)
        {
            return [];
        }
    }

    /// <summary>What `rola key` printed, read into the paths it listed.</summary>
    /// <param name="output">What the command printed, which is one JSON object when it was asked for one.</param>
    /// <returns>The paths, which is none when the answer does not read.</returns>
    internal static IReadOnlyList<string> PathsOf(string output)
    {
        ArgumentNullException.ThrowIfNull(output);

        try
        {
            using var document = JsonDocument.Parse(output);

            if (
                document.RootElement.ValueKind is not JsonValueKind.Object
                || !document.RootElement.TryGetProperty("paths", out var paths)
                || paths.ValueKind is not JsonValueKind.Array
            )
            {
                return [];
            }

            return
            [
                .. paths
                    .EnumerateArray()
                    .Select(path => path.GetString())
                    .Where(path => path is { Length: > 0 })
                    .Select(path => path!),
            ];
        }
        catch (JsonException)
        {
            return [];
        }
    }

    /// <summary>Whether a run ended well, and what to say when it did not.</summary>
    /// <param name="ran">What the run came to.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    private static (bool Done, string Said) Ended((bool Done, string Output, string Said) ran) =>
        (ran.Done, ran.Said);

    /// <summary>Runs a command and waits, answering whether it ended well and what it wrote.</summary>
    /// <remarks>
    /// Both streams are drained while it runs, so that a command that fills one pipe while this waits on the
    /// other cannot block against itself.
    /// </remarks>
    /// <param name="arguments">What to run it with, after the program's name.</param>
    /// <param name="directory">Where to run it, which is the scopes a key is looked for in.</param>
    /// <returns>Whether it ended well, what it wrote, and what it said when it did not.</returns>
    private static (bool Done, string Output, string Said) Run(
        IReadOnlyList<string> arguments,
        string directory
    )
    {
        var program = Environment.GetEnvironmentVariable(Program);

        if (string.IsNullOrEmpty(program))
        {
            return (false, string.Empty, string.Empty);
        }

        var start = new ProcessStartInfo
        {
            FileName = program,
            WorkingDirectory = directory,
            UseShellExecute = false,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            CreateNoWindow = true,
            // The words are `rola`'s own, and `rola` writes UTF-8 — as every stream this project's programs
            // write is. Left unstated, a redirected stream is decoded in the machine's codepage, which on a
            // Chinese Windows is GBK, and every word of the message comes out as mojibake.
            StandardOutputEncoding = Encoding.UTF8,
            StandardErrorEncoding = Encoding.UTF8,
        };

        start.ArgumentList.Add("--theme-choice=" + Theme);

        foreach (var argument in arguments)
        {
            start.ArgumentList.Add(argument);
        }

        try
        {
            using var process =
                Process.Start(start) ?? throw new InvalidOperationException("the process was not started");

            var error = Task.Run(() => process.StandardError.ReadToEnd());
            var output = Task.Run(() => process.StandardOutput.ReadToEnd());

            process.WaitForExit();

            var said = error.GetAwaiter().GetResult().Trim();
            var written = output.GetAwaiter().GetResult().Trim();

            if (process.ExitCode == 0)
            {
                return (true, written, string.Empty);
            }

            return (
                false,
                written,
                said.Length > 0 ? said : written.Length > 0 ? written : $"the command ended with {process.ExitCode}"
            );
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            return (false, string.Empty, error.Message);
        }
    }
}

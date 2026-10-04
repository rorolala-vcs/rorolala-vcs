using FileSystemPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The command template a file operation is carried out by: what it may name, and what it comes to.
/// </summary>
/// <remarks>
/// The template is the user's, so what is checked is the little language it is written in rather than any
/// one command: the two paths, and an environment variable named by <c>{{env:NAME}}</c>, which is how a
/// command names the program it is run beside rather than trusting a <c>PATH</c>.
/// </remarks>
public sealed class CommandTests
{
    /// <summary>The variable the templates in these checks name.</summary>
    private const string Named = "ROROLALA_HOST_TEST_EXE";

    /// <summary>Begins each test from the variable unset.</summary>
    public CommandTests() => Environment.SetEnvironmentVariable(Named, null);

    /// <summary>
    /// A variable a template names is answered with what the environment holds, as one argument.
    /// </summary>
    /// <remarks>
    /// One argument is the whole of why a variable is a token of its own: a program under a directory with a
    /// space in it is one word here, where writing the value into the line would have made two — and the
    /// second would be read as an argument of the command.
    /// </remarks>
    [Fact]
    public void AVariableIsAnsweredWithWhatTheEnvironmentHolds()
    {
        Environment.SetEnvironmentVariable(Named, "/a directory with spaces/rola");

        Assert.Equal(
            ["/a directory with spaces/rola", "fs-ops", "mv", "hero.psd", "/elsewhere/hero.psd"],
            Executor.Arguments(
                [$"{{{{env:{Named}}}}}", "fs-ops", "mv", Executor.From, Executor.To],
                "hero.psd",
                "/elsewhere/hero.psd"
            )
        );
    }

    /// <summary>The two paths are still answered as they were, beside a variable.</summary>
    [Fact]
    public void ThePathsAreAnsweredAsTheyWere()
    {
        Environment.SetEnvironmentVariable(Named, "rola");

        Assert.Equal(
            ["rola", "fs-ops", "rm", "solo.psd"],
            Executor.Arguments(
                [$"{{{{env:{Named}}}}}", "fs-ops", "rm", Executor.From],
                "solo.psd",
                string.Empty
            )
        );
    }

    /// <summary>A template naming a variable nothing is set to is refused, and says which.</summary>
    [Fact]
    public void AVariableNothingIsSetToIsRefused()
    {
        var problem = Executor.Validate(
            Operation.Move,
            $"{{{{env:{Named}}}}} fs-ops mv {Executor.From} {Executor.To}"
        );

        Assert.NotNull(problem);
        Assert.Contains(Named, problem, StringComparison.Ordinal);
    }

    /// <summary>A variable that is set is taken where the program goes, and the command is allowed.</summary>
    [Fact]
    public void AVariableThatIsSetIsTakenWhereTheProgramGoes()
    {
        Environment.SetEnvironmentVariable(Named, "/bin/rola");

        Assert.Null(
            Executor.Validate(
                Operation.Move,
                $"{{{{env:{Named}}}}} fs-ops mv {Executor.From} {Executor.To}"
            )
        );
    }

    /// <summary>A variable named by nothing is refused rather than kept as a word.</summary>
    [Fact]
    public void AVariableNamedByNothingIsRefused()
    {
        Assert.NotNull(
            Executor.Validate(
                Operation.Move,
                $"{{{{env:}}}} fs-ops mv {Executor.From} {Executor.To}"
            )
        );
    }
}

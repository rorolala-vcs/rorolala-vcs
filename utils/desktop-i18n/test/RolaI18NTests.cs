using System.Globalization;

using RorolalaDesktop.I18n;

namespace RorolalaDesktopI18n.Tests;

/// <summary>
/// The translations the Desktop program reads: what a file says, and what the program says when a
/// file says nothing.
/// </summary>
public sealed class RolaI18NTests : IDisposable
{
    /// <summary>The directory each test writes its translation files into.</summary>
    private readonly string _root = Path.Combine(
        Path.GetTempPath(),
        $"rorolala-i18n-{Guid.NewGuid():N}"
    );

    public RolaI18NTests() => Directory.CreateDirectory(_root);

    public void Dispose()
    {
        if (Directory.Exists(_root))
        {
            Directory.Delete(_root, recursive: true);
        }
    }

    /// <summary>A file of the shape the command line keeps its own words in.</summary>
    private const string PackFile = """
        _version: 2

        pack:
          result_packed:
            en: Laid the store's objects out afresh
            zh-CN: 已重新整理仓库中的对象

          result_nothing:
            en: The store was already laid out that way
            zh-CN: 仓库已是这样的布局

          err_pack_failed:
            en: "The store would not pack — %{reason}"
            zh-CN: "仓库无法打包 —— %{reason}"

          help:
            en: |
                Lay a store's objects out in as few packs as its size limit allows

                **Usage**: `rola pack`
            zh-CN: |
                将仓库中的对象整理进尽可能少的 pack
        """;

    /// <summary>
    /// A file stating a node written in both languages and one written in only one of them.
    /// </summary>
    private const string ErrorFile = """
        _version: 2

        error:
          vault:
            err_not_bound:
              en: "`%{name}` is not bound to a Vault"
              zh-CN: "`%{name}` 未绑定到任何保险库"

            err_not_bound_help:
              zh-CN: 运行 `rola vault` 查看已绑定的名字
        """;

    /// <summary>A file whose nodes are only about how a form is filled.</summary>
    private const string FormsFile = """
        _version: 2

        forms:
          order:
            en: "%{late} and %{early}"

          two:
            en: "%{first} then %{second}"

          counted:
            en: "%{count} items"
        """;

    /// <summary>A file written with the other extension a translation may be written with.</summary>
    private const string OtherExtensionFile = """
        _version: 2

        other:
          said:
            en: Said from a `.yaml` file
            zh-CN: 来自 `.yaml` 文件
        """;

    /// <summary>A file that states no node, which is not a mistake.</summary>
    private const string EmptyFile = """
        _version: 2
        """;

    /// <summary>A file no reader can make sense of.</summary>
    private const string BrokenFile = """
        _version: 2

        pack:
          help: "a form that never closes
        """;

    /// <summary>Writes a translation file, making the directories it sits in.</summary>
    private void Given(string relativePath, string content)
    {
        var file = Path.Combine(_root, relativePath);
        var directory = Path.GetDirectoryName(file);

        Directory.CreateDirectory(directory ?? _root);
        File.WriteAllText(file, content);
    }

    /// <summary>Points the subject at the scratch directory and speaks a language.</summary>
    private void Speaking(string locale)
    {
        RolaI18N.SetTranslationDirectory(_root);
        RolaI18N.SetLocale(locale);
    }

    [Fact]
    public void What_was_handed_over_is_readable_back()
    {
        RolaI18N.SetTranslationDirectory(_root);
        RolaI18N.SetLocale("zh-CN");

        Assert.Equal(_root, RolaI18N.TranslationDirectory);
        Assert.Equal("zh-CN", RolaI18N.Locale);
    }

    [Fact]
    public void Every_file_under_the_directory_is_read_and_their_nodes_are_reached_by_key()
    {
        Given("pack.yml", PackFile);
        Given("error/vault.yml", ErrorFile);

        Speaking("en");

        Assert.Equal("Laid the store's objects out afresh", RolaI18N.Get("pack.result_packed"));
        Assert.Equal(
            "`origin` is not bound to a Vault",
            RolaI18N.Get("error.vault.err_not_bound", "origin")
        );
    }

    [Fact]
    public void Where_a_file_sits_says_nothing_about_the_keys_it_states()
    {
        // The keys this file states are the ones it nests, and the path it sits under states none
        // of them: filing it elsewhere changes nothing about what it says.
        Given("filings/whatever/deep.yml", ErrorFile);

        Speaking("en");

        Assert.Equal(
            "`origin` is not bound to a Vault",
            RolaI18N.Get("error.vault.err_not_bound", "origin")
        );
    }

    [Fact]
    public void A_form_is_spoken_in_the_language_the_program_speaks()
    {
        Given("pack.yml", PackFile);

        Speaking("zh-CN");
        Assert.Equal("仓库已是这样的布局", RolaI18N.Get("pack.result_nothing"));

        Speaking("en");
        Assert.Equal("The store was already laid out that way", RolaI18N.Get("pack.result_nothing"));
    }

    [Fact]
    public void A_language_no_form_is_written_in_is_spoken_by_the_fallback()
    {
        Given("pack.yml", PackFile);

        Speaking("de");

        Assert.Equal("The store was already laid out that way", RolaI18N.Get("pack.result_nothing"));
    }

    [Fact]
    public void A_node_written_in_no_language_the_program_speaks_reads_as_its_key()
    {
        Given("error/vault.yml", ErrorFile);

        Speaking("en");
        Assert.Equal(
            "error.vault.err_not_bound_help",
            RolaI18N.Get("error.vault.err_not_bound_help")
        );

        Speaking("zh-CN");
        Assert.Equal(
            "运行 `rola vault` 查看已绑定的名字",
            RolaI18N.Get("error.vault.err_not_bound_help")
        );
    }

    [Fact]
    public void A_key_no_file_states_reads_as_the_key()
    {
        Given("pack.yml", PackFile);

        Speaking("en");

        Assert.Equal("no.such.key", RolaI18N.Get("no.such.key"));
    }

    [Fact]
    public void A_form_written_as_a_block_is_read_without_the_newline_it_ends_with()
    {
        Given("pack.yml", PackFile);

        Speaking("en");
        var help = RolaI18N.Get("pack.help");

        Assert.StartsWith("Lay a store's objects out", help, StringComparison.Ordinal);
        Assert.EndsWith("**Usage**: `rola pack`", help, StringComparison.Ordinal);
    }

    [Fact]
    public void Values_go_into_the_places_by_order_and_not_by_name()
    {
        Given("forms.yml", FormsFile);

        Speaking("en");

        // The places are called `late` and `early` and the values arrive the other way round: what
        // a place is called is for whoever reads the file, and what it is given is its turn.
        Assert.Equal("first and second", RolaI18N.Get("forms.order", "first", "second"));
        Assert.Equal("one then two", RolaI18N.Get("forms.two", "one", "two"));
    }

    [Fact]
    public void A_place_left_without_a_value_is_left_as_it_was_written()
    {
        Given("pack.yml", PackFile);

        Speaking("en");

        Assert.Contains("%{reason}", RolaI18N.Get("pack.err_pack_failed"));
    }

    [Fact]
    public void A_value_left_without_a_place_is_dropped()
    {
        Given("pack.yml", PackFile);

        Speaking("en");

        Assert.Equal(
            "The store was already laid out that way",
            RolaI18N.Get("pack.result_nothing", "one", "two", "three")
        );
    }

    [Fact]
    public void A_value_is_written_the_same_in_every_language()
    {
        Given("forms.yml", FormsFile);

        Speaking("en");

        // A language that writes a decimal with a comma must not change what a form says, since the
        // command line writes its values without consulting a language at all.
        var before = CultureInfo.CurrentCulture;
        CultureInfo.CurrentCulture = CultureInfo.GetCultureInfo("de-DE");

        try
        {
            Assert.Equal("1.5 items", RolaI18N.Get("forms.counted", 1.5));
        }
        finally
        {
            CultureInfo.CurrentCulture = before;
        }
    }

    [Fact]
    public void A_language_is_named_as_a_locale()
    {
        Given("pack.yml", PackFile);

        RolaI18N.SetTranslationDirectory(_root);
        RolaI18N.SetLocale("zh_CN.UTF-8");

        Assert.Equal("zh-CN", RolaI18N.Locale);
        Assert.Equal("仓库已是这样的布局", RolaI18N.Get("pack.result_nothing"));
    }

    [Fact]
    public void Both_extensions_are_read_and_a_file_that_is_neither_is_left_alone()
    {
        Given("other.yaml", OtherExtensionFile);
        Given("notes.txt", "pack:\n  nothing:\n    en: read me not\n");
        Given("nothing.yml", EmptyFile);

        Speaking("en");

        Assert.Equal("Said from a `.yaml` file", RolaI18N.Get("other.said"));

        // The words are in the directory and would read as a node were the file read at all.
        Assert.Equal("pack.nothing", RolaI18N.Get("pack.nothing"));
    }

    [Fact]
    public void The_version_a_file_opens_with_is_not_a_node()
    {
        Given("pack.yml", PackFile);

        Speaking("en");

        Assert.Equal("_version", RolaI18N.Get("_version"));
    }

    [Fact]
    public void Naming_another_directory_reads_the_files_again()
    {
        Given("one/pack.yml", PackFile);
        Given("two/other.yaml", OtherExtensionFile);

        RolaI18N.SetLocale("en");

        RolaI18N.SetTranslationDirectory(Path.Combine(_root, "one"));
        Assert.Equal("pack.other.said", RolaI18N.Get("pack.other.said"));
        Assert.Equal("Laid the store's objects out afresh", RolaI18N.Get("pack.result_packed"));

        RolaI18N.SetTranslationDirectory(Path.Combine(_root, "two"));
        Assert.Equal("Said from a `.yaml` file", RolaI18N.Get("other.said"));
        Assert.Equal("pack.result_packed", RolaI18N.Get("pack.result_packed"));
    }

    [Fact]
    public void A_directory_that_is_not_there_is_refused()
    {
        RolaI18N.SetTranslationDirectory(Path.Combine(_root, "not-there"));

        Assert.Throws<DirectoryNotFoundException>(() => RolaI18N.Get("pack.help"));
    }

    [Fact]
    public void A_file_that_does_not_read_is_refused_and_the_file_is_named()
    {
        Given("broken.yml", BrokenFile);

        Speaking("en");

        var refused = Assert.Throws<FormatException>(() => RolaI18N.Get("pack.help"));

        Assert.Contains("broken.yml", refused.Message);
    }

    [Fact]
    public void Nothing_is_asked_for_or_named_under_an_empty_name()
    {
        Given("pack.yml", PackFile);

        Speaking("en");

        Assert.Throws<ArgumentException>(() => RolaI18N.Get(""));
        Assert.Throws<ArgumentException>(() => RolaI18N.SetLocale(""));
        Assert.Throws<ArgumentException>(() => RolaI18N.SetTranslationDirectory(""));
    }
}

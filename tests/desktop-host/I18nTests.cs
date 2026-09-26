using RorolalaDesktop.I18n;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The translations, read from several directories at once.
/// </summary>
/// <remarks>
/// The host reads its own directory first and each plugin adds its own after, so the merge rule is
/// what keeps a plugin from overwriting a word the host says.
/// </remarks>
public sealed class I18nTests
{
    /// <summary>A key an earlier directory states is not replaced by a later one.</summary>
    [Fact]
    public void ALaterDirectoryDoesNotReplaceAKeyAnEarlierOneStates()
    {
        var first = Scratch.Translations("first", "shared:\n  en: from the first\n");
        var second = Scratch.Translations(
            "second",
            "shared:\n  en: from the second\nsecond_only:\n  en: second only\n"
        );

        RolaI18N.SetTranslationDirectory(first);
        RolaI18N.RegisterTranslationDirectory(second);

        Assert.Equal("from the first", RolaI18N.Get("shared"));
        Assert.Equal("second only", RolaI18N.Get("second_only"));
    }

    /// <summary>A directory registered twice is one directory, and its keys do not change.</summary>
    [Fact]
    public void ADirectoryRegisteredTwiceIsOneDirectory()
    {
        var first = Scratch.Translations("first", "shared:\n  en: from the first\n");

        RolaI18N.SetTranslationDirectory(first);
        RolaI18N.RegisterTranslationDirectory(first);

        Assert.Single(RolaI18N.TranslationDirectories);
        Assert.Equal("from the first", RolaI18N.Get("shared"));
    }

    /// <summary>Naming one directory replaces them all, which is what the host does at startup.</summary>
    [Fact]
    public void NamingOneDirectoryReplacesThemAll()
    {
        var first = Scratch.Translations("first", "shared:\n  en: from the first\n");
        var second = Scratch.Translations("second", "shared:\n  en: from the second\n");

        RolaI18N.SetTranslationDirectory(first);
        RolaI18N.RegisterTranslationDirectory(second);
        RolaI18N.SetTranslationDirectory(second);

        Assert.Equal("from the second", RolaI18N.Get("shared"));
        Assert.Single(RolaI18N.TranslationDirectories);
    }
}

using FileSystemPlugin;
using RorolalaDesktop.Contract;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// Renaming a name: how much of it a rename chooses when it begins.
/// </summary>
/// <remarks>
/// The box a name is edited in is built out of controls the toolkit draws, so what is checked here is the
/// rule the choice is made by rather than the box: what a user sees when a rename begins is a name chosen up
/// to its extension, and that is what a wrong rule would get wrong.
/// </remarks>
public sealed class RenameTests
{
    /// <summary>A file's extension is left out of what is chosen, so typing replaces the name alone.</summary>
    /// <param name="name">The name being renamed.</param>
    /// <param name="chosen">Where the chosen part is to end.</param>
    [Theory]
    [InlineData("hero.psd", 4)]
    // The extension is what the last dot begins, which is what a program that opens a file by its extension
    // reads: `archive.tar.gz` is a gzipped tar rather than a file of a `.tar.gz` kind.
    [InlineData("archive.tar.gz", 11)]
    [InlineData("a.b", 1)]
    // The dot is the whole of the extension: what is left is a name and the beginning of one.
    [InlineData("name.", 4)]
    public void AFileNameIsChosenUpToItsExtension(string name, int chosen) =>
        Assert.Equal(chosen, Names.Chosen(EntryKind.File, name));

    /// <summary>What has no extension to leave out is chosen whole.</summary>
    /// <remarks>
    /// A dot-file is the important one: the dot that begins it is part of the name rather than the start of
    /// an extension, so choosing up to it would choose nothing at all.
    /// </remarks>
    /// <param name="name">The name being renamed.</param>
    [Theory]
    [InlineData(".gitignore")]
    [InlineData("README")]
    [InlineData("")]
    public void WhatHasNoExtensionIsChosenWhole(string name) =>
        Assert.Equal(name.Length, Names.Chosen(EntryKind.File, name));

    /// <summary>A directory is chosen whole, whatever dots its name has.</summary>
    /// <param name="name">The name being renamed.</param>
    [Theory]
    [InlineData("art.v2")]
    [InlineData("New Folder")]
    [InlineData("scenes.2024.final")]
    public void ADirectoryIsChosenWhole(string name) =>
        Assert.Equal(name.Length, Names.Chosen(EntryKind.Directory, name));

    /// <summary>
    /// A second of a taken name carries the number before its extension rather than after it.
    /// </summary>
    /// <remarks>
    /// The two are one rule read from its two ends — where a name stops and what says the kind of file it is
    /// begin at the same dot — so a name that took its number at the end is a name whose extension is the
    /// number, which is a file nothing opens by its extension any more.
    /// </remarks>
    /// <param name="name">The name that is taken.</param>
    /// <param name="numbered">What the second of it is called.</param>
    [Theory]
    [InlineData("源文件.txt", "源文件 (2).txt")]
    [InlineData("hero.psd", "hero (2).psd")]
    // The last dot is where the extension begins, so a stack of them keeps all but the last.
    [InlineData("源文件.pkg.tar.zst", "源文件.pkg.tar (2).zst")]
    [InlineData("archive.tar.gz", "archive.tar (2).gz")]
    [InlineData("a.b", "a (2).b")]
    // The dot is the whole of the extension, and what precedes it is the name.
    [InlineData("name.", "name (2).")]
    public void ANumberGoesBeforeTheExtension(string name, string numbered) =>
        Assert.Equal(numbered, Names.Numbered(name, 2));

    /// <summary>
    /// What has no extension to put a number before takes it at the end.
    /// </summary>
    /// <param name="name">The name that is taken.</param>
    /// <param name="numbered">What the second of it is called.</param>
    [Theory]
    [InlineData("README", "README (2)")]
    [InlineData(".gitignore", ".gitignore (2)")]
    [InlineData("", " (2)")]
    public void WhatHasNoExtensionTakesTheNumberAtItsEnd(string name, string numbered) =>
        Assert.Equal(numbered, Names.Numbered(name, 2));
}

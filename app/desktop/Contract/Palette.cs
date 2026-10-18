using Avalonia.Media;

namespace RorolalaDesktop.Contract;

/// <summary>
/// The colours everything is drawn with, as the look in force holds them.
/// </summary>
/// <remarks>
/// One palette for the program rather than one per plugin: what a surface is coloured, what is written on it,
/// and what tells it from the next are one answer each, and a plugin that picked its own would be a plugin that
/// disagreed with the window it is in the moment the user chose different colours. What a plugin contributes is
/// which of these a thing wears — a role — and never the colour itself.
/// <para>
/// Five of them are what a surface is made of: the plane it stands on, the face it is, the hollow it sits in,
/// the ink on it, and the line that tells it from what is beside it. Which of the two looks the five come
/// from is the theme's answer — the dark one, the light one, or whichever the system is in — and a plugin is
/// never told which: it asks for the face, and the face is what it is on the ground in force.
/// </para>
/// <para>
/// The other three are the colours the user chooses for the program itself: what is chosen, the second
/// colour spent on what must not read as a choice, and what is written on the primary. They do not follow
/// the window's variant, and a plugin that draws a mark in the program's own colours asks for them here
/// rather than naming a colour of its own.
/// </para>
/// <para>
/// What a plugin says about its own marks — a state of its own, which is not a colour the program has an
/// opinion about — is the plugin's to hold, and is not here.
/// </para>
/// </remarks>
public interface IPalette
{
    /// <summary>The face a card is: what a surface that stands out from the ground is filled with.</summary>
    Color Bright { get; }

    /// <summary>The ground itself: what the program is drawn on where nothing stands on anything.</summary>
    Color Mid { get; }

    /// <summary>The hollow: what a surface that sits in the ground — an input, a well — is filled with.</summary>
    Color Dark { get; }

    /// <summary>The ink: what is written on the three of them.</summary>
    Color Front { get; }

    /// <summary>The line: what tells a surface from the ground, and one region from the next.</summary>
    Color Splitter { get; }

    /// <summary>What is chosen, in the colour the user set for the program.</summary>
    Color Primary { get; }

    /// <summary>The second colour, spent on what must not read as a choice.</summary>
    Color Accent { get; }

    /// <summary>What is written on <see cref="Primary"/>.</summary>
    Color PrimaryText { get; }
}

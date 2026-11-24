namespace RorolalaDesktop.Contract;

/// <summary>How much of the program's motion is drawn.</summary>
/// <remarks>
/// It is one choice for every surface, the host's own and a plugin's: a person who needs less motion needs it
/// everywhere at once, and a surface that kept its own answer would be a place the choice did not reach.
/// </remarks>
public enum MotionLevel
{
    /// <summary>Every movement the program has.</summary>
    Full,

    /// <summary>The fades, without the travel and the scaling.</summary>
    Reduced,

    /// <summary>Nothing is animated: every change arrives at once.</summary>
    Off,
}

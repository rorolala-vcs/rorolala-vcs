namespace RorolalaDesktop.Contract;

/// <summary>
/// Where a plugin keeps what it can make again.
/// </summary>
/// <remarks>
/// A reading of something that is read often and costs something to read belongs here rather than in
/// the settings: a cache is thrown away when it is wrong without anybody being asked, and a setting
/// is a decision a person made. What is written is the plugin's own business — the host names the
/// directory and asks nothing about what is in it.
/// <para>
/// The directory is the plugin's alone, so a file in it never has to be named after the plugin: two
/// plugins that both keep <c>tree.json</c> keep it in their own.
/// </para>
/// </remarks>
public interface ICache
{
    /// <summary>The directory the plugin may keep its cache in, made when it is first asked for.</summary>
    /// <remarks>
    /// The directory stands under the program's own data directory rather than under a temporary one:
    /// a cache is read on the next run, which a temporary directory is not promised to survive.
    /// </remarks>
    string Directory { get; }
}

using RorolalaDesktop.Contract;

namespace FileSystemPlugin;

/// <summary>
/// A place a plugin answers for, as the browser reads it: the address, who answers it, and what is in it.
/// </summary>
/// <remarks>
/// The answer to one address, read once so that everything drawn about it agrees: the title on the path bar, the
/// entries in the listing, the step over it, and the provider that a drag is offered to. A provider of nothing is
/// a place nobody answers — an address whose prefix no plugin claimed — which is a thing to say rather than a
/// failure to read.
/// </remarks>
/// <param name="Address">The address that was opened.</param>
/// <param name="Provider">What answers it, or nothing when no plugin does.</param>
/// <param name="Title">What the place is called, or the address itself when nobody answers it.</param>
/// <param name="Entries">What is in it, which is nothing when nobody answers it.</param>
/// <param name="Parent">Where going up from it goes, or nothing when there is nowhere up.</param>
internal sealed record LogicalPlace(
    LogicalAddress Address,
    ILogicalViewProvider? Provider,
    string Title,
    IReadOnlyList<LogicalEntry> Entries,
    LogicalAddress? Parent
);

/// <summary>
/// What the path bar comes to, and what a drag onto it comes to.
/// </summary>
/// <remarks>
/// The reading side of the logical-view point: what was typed is either an address or a path, an address is
/// either answered or not, and a drag is offered to whatever answers the place it was dropped on. Nothing here
/// draws anything, which is what lets all of it be checked without a window.
/// </remarks>
internal static class Logical
{
    /// <summary>
    /// Reads what was typed into the path bar.
    /// </summary>
    /// <remarks>
    /// A path is not an address and an address is not a path: what comes back is a place only when what was typed
    /// is of the shape an address is, so a directory whose name happens to hold an <c>@</c> still opens as the
    /// directory it is. The address is asked for its entries here rather than by whoever draws them, so that one
    /// answer is what the whole place is drawn from.
    /// </remarks>
    /// <param name="written">What was typed.</param>
    /// <param name="registry">What the plugins claimed.</param>
    /// <returns>The place, or nothing when what was typed is a path.</returns>
    public static LogicalPlace? Find(string written, ILogicalViewRegistry registry)
    {
        ArgumentNullException.ThrowIfNull(registry);

        if (!LogicalAddress.TryRead(written, out var address) || address is null)
        {
            return null;
        }

        var provider = registry.Find(address.Scheme);

        return provider is null
            ? new LogicalPlace(address, null, address.ToString(), [], null)
            : new LogicalPlace(
                address,
                provider,
                provider.Title(address),
                provider.Entries(address),
                provider.Parent(address)
            );
    }

    /// <summary>
    /// Whether the place takes what is being dragged over it.
    /// </summary>
    /// <remarks>
    /// Asked while the pointer is still moving, which is what decides whether the drop is offered at all. A place
    /// nobody answers takes nothing, and so does one whose provider says no: what is being dropped is the source's
    /// own object, and whether it can be taken is the answerer's question.
    /// </remarks>
    /// <param name="place">The place being dropped on.</param>
    /// <param name="data">What the source made.</param>
    /// <returns>Whether it can be taken.</returns>
    public static bool Accepts(LogicalPlace place, object data)
    {
        ArgumentNullException.ThrowIfNull(place);
        ArgumentNullException.ThrowIfNull(data);

        return place.Provider?.Accepts(place.Address, new LogicalDrag(data)) ?? false;
    }

    /// <summary>Takes a drag that <see cref="Accepts"/> answered for.</summary>
    /// <param name="place">The place being dropped on.</param>
    /// <param name="data">What the source made.</param>
    public static void Drop(LogicalPlace place, object data)
    {
        ArgumentNullException.ThrowIfNull(place);
        ArgumentNullException.ThrowIfNull(data);

        place.Provider?.Drop(place.Address, new LogicalDrag(data));
    }
}

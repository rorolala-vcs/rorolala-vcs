using Xunit.Abstractions;
using Xunit.Sdk;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// Runs the tree panel's checks before the classes that operate on files through the host.
/// </summary>
/// <remarks>
/// The panel reads a tree through the host's native side. A file operation another class starts is a
/// task of its own, and the test that started it can finish while the operation still holds that
/// side, which is what a later panel read would wait on. Running the panel first makes its reading
/// the first one in the process rather than one after an operation the process no longer owns.
/// </remarks>
public sealed class TreePanelFirst : ITestCollectionOrderer
{
    /// <inheritdoc />
    public IEnumerable<ITestCollection> OrderTestCollections(
        IEnumerable<ITestCollection> testCollections
    ) =>
        testCollections
            .OrderBy(
                collection =>
                    collection.DisplayName.Contains("TreePanelTests", StringComparison.Ordinal)
                        ? 0
                        : 1
            )
            .ThenBy(collection => collection.DisplayName, StringComparer.Ordinal);
}

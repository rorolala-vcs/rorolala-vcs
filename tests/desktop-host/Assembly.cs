// The subject of these tests is the host, whose configuration directory and translations are state
// of the process rather than of an object, so every test reads and writes the same state and two of
// them running at once would see each other's.
[assembly: CollectionBehavior(DisableTestParallelization = true)]

// The tree panel reads a tree through the host's native side, and a file operation another class
// left running can still hold that side when the panel's first read is made. The panel is checked
// before those classes so what it reads is its own tree rather than a lock left behind.
[assembly: TestCollectionOrderer(
    "RorolalaDesktopHost.IntegrationTests.TreePanelFirst",
    "RorolalaDesktopHost.IntegrationTests"
)]

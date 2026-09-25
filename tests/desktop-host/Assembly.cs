// The subject of these tests is the host, whose configuration directory and translations are state
// of the process rather than of an object, so every test reads and writes the same state and two of
// them running at once would see each other's.
[assembly: CollectionBehavior(DisableTestParallelization = true)]

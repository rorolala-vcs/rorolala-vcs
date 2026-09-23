// The subject of these tests holds its state in statics — which directory the files are read from
// and which language is spoken belong to the program, not to an object — so every test reads and
// writes the same state, and two of them running at once would see each other's.
[assembly: CollectionBehavior(DisableTestParallelization = true)]

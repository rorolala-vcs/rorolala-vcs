namespace RolaSharp.V2;

/// <summary>A scope that has a VCS index.</summary>
public interface IVCSIndexAccessable
{
    /// <summary>The VCS index the scope keeps.</summary>
    VCSIndexScoop VCSIndex { get; }
}

/// <summary>A scope that has a store.</summary>
public interface IStorageAccessable
{
    /// <summary>The store the scope keeps.</summary>
    StorageScoop Storage { get; }
}

/// <summary>A scope that has a Layout to work from.</summary>
public interface ILayoutAccessable
{
    /// <summary>The Layout the scope works from.</summary>
    LayoutScoop Layout { get; }
}

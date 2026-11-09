using System.Threading;
using System.Threading.Tasks;

namespace RorolalaDesktop.Contract;

/// <summary>How large a preview is asked to be, in pixels.</summary>
/// <param name="Width">The width.</param>
/// <param name="Height">The height.</param>
public readonly record struct PreviewSize(int Width, int Height);

/// <summary>Why a preview is being asked for, which decides how soon it is worth making.</summary>
public enum PreviewPurpose
{
    /// <summary>A grid tile, requested for many visible entries at once.</summary>
    Tile,

    /// <summary>A larger, user-requested preview, which has priority over tiles.</summary>
    Pane,

    /// <summary>A prefetch that may be dropped when the visible work needs the worker.</summary>
    Prefetch,
}

/// <summary>One preview to make.</summary>
/// <param name="Path">The file.</param>
/// <param name="Kind">Whether it is a file or a directory.</param>
/// <param name="Size">How large the preview is asked to be.</param>
/// <param name="Purpose">Why it is being asked for.</param>
public sealed record PreviewRequest(
    string Path,
    EntryKind Kind,
    PreviewSize Size,
    PreviewPurpose Purpose
);

/// <summary>Which layer answered a request.</summary>
public enum PreviewOrigin
{
    /// <summary>The operating system's own thumbnail.</summary>
    System,

    /// <summary>A plugin's renderer.</summary>
    Plugin,
}

/// <summary>
/// Raw pixels, top-down BGRA, so the source need not touch Avalonia's UI thread.
/// </summary>
/// <param name="Width">The width in pixels.</param>
/// <param name="Height">The height in pixels.</param>
/// <param name="Stride">How many bytes one row takes.</param>
/// <param name="Pixels">The bytes, starting at the top row.</param>
/// <param name="Origin">Which layer made it.</param>
public sealed record PreviewImage(
    int Width,
    int Height,
    int Stride,
    ReadOnlyMemory<byte> Pixels,
    PreviewOrigin Origin
);

/// <summary>
/// One layer that may make a preview: the operating system's thumbnailer, or a plugin's renderer.
/// </summary>
/// <remarks>
/// A source is asked in registration order, the system's before any plugin's. <see cref="Cares"/> must be cheap:
/// it is asked for every tile before anything is read, so it may look at an extension or the platform and no more.
/// </remarks>
public interface IPreviewSource
{
    /// <summary>The stable value this source is known by, unique among the sources offered.</summary>
    string Id { get; }

    /// <summary>Whether the source may answer for this request, cheaply.</summary>
    /// <param name="request">The request.</param>
    /// <returns>Whether it is worth asking <see cref="RenderAsync"/>.</returns>
    bool Cares(PreviewRequest request);

    /// <summary>The preview, or nothing when this source cannot make one.</summary>
    /// <param name="request">The request.</param>
    /// <param name="cancel">Cancels the work.</param>
    /// <returns>The pixels, or nothing.</returns>
    ValueTask<PreviewImage?> RenderAsync(PreviewRequest request, CancellationToken cancel);
}

/// <summary>Where a plugin adds a preview renderer.</summary>
public interface IPreviewRegistry
{
    /// <summary>Adds a source after the ones already registered.</summary>
    /// <param name="source">The source.</param>
    void Register(IPreviewSource source);
}

/// <summary>
/// The program's previews: system sources first, then the plugins', with the answers kept for a while.
/// </summary>
public interface IPreviewService
{
    /// <summary>The best preview for a request, or nothing when no source can make one.</summary>
    /// <param name="request">The request.</param>
    /// <param name="cancel">Cancels the work.</param>
    /// <returns>The preview, or nothing.</returns>
    Task<PreviewImage?> Get(PreviewRequest request, CancellationToken cancel);

    /// <summary>Drops what is cached for a path, after a file changed.</summary>
    /// <param name="path">The path.</param>
    void Forget(string path);
}

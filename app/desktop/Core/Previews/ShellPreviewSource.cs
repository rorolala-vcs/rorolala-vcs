using System.Collections.Concurrent;
using System.Runtime.InteropServices;
using System.Runtime.Versioning;
using RorolalaDesktop.Contract;

namespace RorolalaDesktop.Previews;

/// <summary>
/// The operating system's own thumbnails, through the Windows Shell.
/// </summary>
/// <remarks>
/// A Shell thumbnail may run a third party's thumbnail handler, and those are written for a single threaded
/// apartment: calling one from a thread pool thread can hang the pool or fail with a wrong-thread error. Every
/// request therefore goes through one dedicated STA thread, which is also what keeps a slow handler from holding a
/// worker the rest of the program needs.
/// <para>
/// The request asks for a thumbnail only. A file the Shell has no thumbnail for answers nothing, and the plugin
/// renderers are asked after it, which is why an ordinary text file keeps its type icon.
/// </para>
/// </remarks>
[SupportedOSPlatform("windows")]
public sealed class ShellPreviewSource : IPreviewSource, IDisposable
{
    /// <summary>The one thread every thumbnail is made on.</summary>
    private readonly BlockingCollection<Job> _queue = new();

    /// <summary>Makes the source and starts its thread.</summary>
    public ShellPreviewSource()
    {
        var worker = new Thread(Work)
        {
            IsBackground = true,
            Name = "preview-shell",
        };

        worker.SetApartmentState(ApartmentState.STA);
        worker.Start();
    }

    /// <inheritdoc />
    public string Id => "host.shell";

    /// <inheritdoc />
    public bool Cares(PreviewRequest request)
    {
        ArgumentNullException.ThrowIfNull(request);

        return OperatingSystem.IsWindows()
            && request.Kind == EntryKind.File
            && request.Size.Width > 0
            && request.Size.Height > 0
            && File.Exists(request.Path);
    }

    /// <inheritdoc />
    public ValueTask<PreviewImage?> RenderAsync(PreviewRequest request, CancellationToken cancel)
    {
        ArgumentNullException.ThrowIfNull(request);

        var job = new Job(request, cancel);

        if (cancel.IsCancellationRequested)
        {
            job.Answer.TrySetResult(null);

            return new ValueTask<PreviewImage?>(job.Answer.Task);
        }

        try
        {
            _queue.Add(job, cancel);
        }
        catch (OperationCanceledException)
        {
            job.Answer.TrySetResult(null);
        }

        return new ValueTask<PreviewImage?>(job.Answer.Task);
    }

    /// <summary>Stops the worker once the program is done with the source.</summary>
    public void Dispose()
    {
        _queue.CompleteAdding();
        _queue.Dispose();
    }

    /// <summary>Takes one job at a time off the queue and makes its thumbnail.</summary>
    private void Work()
    {
        foreach (var job in _queue.GetConsumingEnumerable())
        {
            if (job.Cancel.IsCancellationRequested)
            {
                job.Answer.TrySetResult(null);

                continue;
            }

            try
            {
                job.Answer.TrySetResult(Thumbnail(job.Request));
            }
            catch (Exception error) when (error is not OutOfMemoryException)
            {
                job.Answer.TrySetException(error);
            }
        }
    }

    /// <summary>Asks the Shell for one thumbnail and copies its pixels out.</summary>
    /// <param name="request">The request.</param>
    /// <returns>The pixels, or nothing when the Shell has no thumbnail.</returns>
    private static PreviewImage? Thumbnail(PreviewRequest request)
    {
        var factory = (IShellItemImageFactory)ShellItem(request.Path);
        var size = new SIZE { cx = request.Size.Width, cy = request.Size.Height };

        try
        {
            var result = factory.GetImage(
                size,
                SIIGBF.ThumbnailOnly | SIIGBF.BiggerSizeOk | SIIGBF.ResizeToFit,
                out var bitmap
            );

            return result < 0 ? null : Pixels(bitmap);
        }
        finally
        {
            Marshal.FinalReleaseComObject(factory);
        }
    }

    /// <summary>Makes the Shell's item for a path.</summary>
    /// <param name="path">The path.</param>
    /// <returns>The item, which the caller releases.</returns>
    private static object ShellItem(string path)
    {
        var id = typeof(IShellItemImageFactory).GUID;

        SHCreateItemFromParsingName(path, IntPtr.Zero, id, out var item);

        return item;
    }

    /// <summary>Copies a device dependent bitmap out as top-down BGRA.</summary>
    /// <param name="bitmap">The bitmap the Shell gave.</param>
    /// <returns>The pixels, or nothing when it is not a 32 bit bitmap.</returns>
    private static PreviewImage? Pixels(IntPtr bitmap)
    {
        if (bitmap == IntPtr.Zero)
        {
            return null;
        }

        try
        {
            var header = new BITMAP();
            var got = GetObject(bitmap, Marshal.SizeOf<BITMAP>(), ref header);

            if (got == 0 || header.bmWidth <= 0 || header.bmHeight <= 0)
            {
                return null;
            }

            var width = header.bmWidth;
            var height = header.bmHeight;
            var stride = width * 4;
            var pixels = new byte[stride * height];

            var info = new BITMAPINFO
            {
                bmiHeader = new BITMAPINFOHEADER
                {
                    biSize = Marshal.SizeOf<BITMAPINFOHEADER>(),
                    biWidth = width,
                    // A negative height asks for the rows from the top, which is what the view wants.
                    biHeight = -height,
                    biPlanes = 1,
                    biBitCount = 32,
                    biCompression = 0,
                },
            };

            var context = CreateCompatibleDC(IntPtr.Zero);

            try
            {
                var previous = SelectObject(context, bitmap);

                try
                {
                    var rows = GetDIBits(
                        context,
                        bitmap,
                        0,
                        (uint)height,
                        pixels,
                        ref info,
                        DIB_RGB_COLORS
                    );

                    return rows == height
                        ? new PreviewImage(width, height, stride, pixels, PreviewOrigin.System)
                        : null;
                }
                finally
                {
                    SelectObject(context, previous);
                }
            }
            finally
            {
                DeleteDC(context);
            }
        }
        finally
        {
            DeleteObject(bitmap);
        }
    }

    /// <summary>One request waiting for the one STA thread.</summary>
    /// <param name="Request">What to make.</param>
    /// <param name="Cancel">Cancels the wait.</param>
    private sealed record Job(PreviewRequest Request, CancellationToken Cancel)
    {
        /// <summary>What the waiting caller is told.</summary>
        public TaskCompletionSource<PreviewImage?> Answer { get; } =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
    }

    /// <summary>The Shell's item image factory.</summary>
    [ComImport]
    [Guid("bcc18b79-ba16-442f-80c4-8a59c30c463b")]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    private interface IShellItemImageFactory
    {
        [PreserveSig]
        int GetImage(SIZE size, SIIGBF flags, out IntPtr bitmap);
    }

    /// <summary>The size the Shell is asked for.</summary>
    [StructLayout(LayoutKind.Sequential)]
    private struct SIZE
    {
        public int cx;
        public int cy;
    }

    /// <summary>What the Shell may do with the picture.</summary>
    [Flags]
    private enum SIIGBF
    {
        ResizeToFit = 0x00,
        BiggerSizeOk = 0x01,
        ThumbnailOnly = 0x08,
    }

    /// <summary>The Windows bitmap header.</summary>
    [StructLayout(LayoutKind.Sequential)]
    private struct BITMAP
    {
        public int bmType;
        public int bmWidth;
        public int bmHeight;
        public int bmWidthBytes;
        public ushort bmPlanes;
        public ushort bmBitsPixel;
        public IntPtr bmBits;
    }

    /// <summary>The bitmap info the pixels are read through.</summary>
    [StructLayout(LayoutKind.Sequential)]
    private struct BITMAPINFO
    {
        public BITMAPINFOHEADER bmiHeader;
        public uint bmiColors;
    }

    /// <summary>One bitmap's shape and format.</summary>
    [StructLayout(LayoutKind.Sequential)]
    private struct BITMAPINFOHEADER
    {
        public int biSize;
        public int biWidth;
        public int biHeight;
        public ushort biPlanes;
        public ushort biBitCount;
        public int biCompression;
        public int biSizeImage;
        public int biXPelsPerMeter;
        public int biYPelsPerMeter;
        public int biClrUsed;
        public int biClrImportant;
    }

    private const uint DIB_RGB_COLORS = 0;

    [DefaultDllImportSearchPaths(DllImportSearchPath.System32)]
    [DllImport("shell32.dll", CharSet = CharSet.Unicode, PreserveSig = false)]
    private static extern void SHCreateItemFromParsingName(
        string path,
        IntPtr bind,
        [MarshalAs(UnmanagedType.LPStruct)] Guid id,
        [MarshalAs(UnmanagedType.Interface)] out object item
    );

    [DefaultDllImportSearchPaths(DllImportSearchPath.System32)]
    [DllImport("gdi32.dll")]
    private static extern int GetObject(IntPtr handle, int size, ref BITMAP bitmap);

    [DefaultDllImportSearchPaths(DllImportSearchPath.System32)]
    [DllImport("gdi32.dll")]
    private static extern IntPtr CreateCompatibleDC(IntPtr reference);

    [DefaultDllImportSearchPaths(DllImportSearchPath.System32)]
    [DllImport("gdi32.dll")]
    private static extern IntPtr SelectObject(IntPtr context, IntPtr handle);

    [DefaultDllImportSearchPaths(DllImportSearchPath.System32)]
    [DllImport("gdi32.dll")]
    private static extern bool DeleteObject(IntPtr handle);

    [DefaultDllImportSearchPaths(DllImportSearchPath.System32)]
    [DllImport("gdi32.dll")]
    private static extern bool DeleteDC(IntPtr context);

    [DefaultDllImportSearchPaths(DllImportSearchPath.System32)]
    [DllImport("gdi32.dll")]
    private static extern int GetDIBits(
        IntPtr context,
        IntPtr bitmap,
        uint start,
        uint rows,
        byte[] pixels,
        ref BITMAPINFO info,
        uint usage
    );
}

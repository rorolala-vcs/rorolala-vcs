using System.Runtime.InteropServices;
using System.Runtime.Versioning;
using Avalonia;
using Avalonia.Media.Imaging;
using Avalonia.Platform;

namespace RorolalaDesktop.SysIcons;

/// <summary>
/// The icon the Windows shell gives a file or a directory.
/// </summary>
/// <remarks>
/// The shell is the only thing on Windows that knows what a folder looks like, and it answers with an
/// icon handle rather than with a picture, so the handle is turned into pixels here.
/// <para>
/// The shell is asked about a name and an attribute rather than about a path, which is what makes it
/// answer with the icon for a folder as such and for a plain file as such: nothing has to exist on
/// disk, and every directory gets the same picture, which is what an icon for a kind of thing is.
/// </para>
/// <para>
/// The shell keeps that picture at four sizes and the one wanted is read from the list that holds it,
/// rather than the one size there is and a stretch afterward: a grid whose icons are drawn at the
/// size the zoom asks for is drawn from the size the shell has, and only a size the shell does not
/// keep is scaled at all.
/// </para>
/// <para>
/// This cannot be run where it is written — it is Windows-only code on a machine that has no Windows —
/// and it is written so that being wrong costs a placeholder icon rather than a broken listing. What
/// it is written on is said by the attribute below rather than by a comment, so that the compiler
/// checks it holds of every caller.
/// </para>
/// </remarks>
[SupportedOSPlatform("windows")]
internal static class ShellIcon
{
    /// <summary>The shell's icon for one kind of thing, at a size, or nothing.</summary>
    /// <param name="kind">The kind of thing the icon is for.</param>
    /// <param name="size">How many pixels wide and tall the icon is wanted.</param>
    public static Bitmap? Drawn(Kind kind, int size)
    {
        var info = default(SHFILEINFO);

        var answered = SHGetFileInfo(
            kind == Kind.Directory ? "directory" : "file.txt",
            kind == Kind.Directory ? FileAttributeDirectory : FileAttributeNormal,
            ref info,
            (uint)Marshal.SizeOf<SHFILEINFO>(),
            Icon | SysIconIndex | UseFileAttributes
        );

        if (answered == IntPtr.Zero || info.Icon == IntPtr.Zero)
        {
            return null;
        }

        try
        {
            return Listed(info.Index, size) ?? Pixels(info.Icon);
        }
        finally
        {
            DestroyIcon(info.Icon);
        }
    }

    /// <summary>
    /// The shell's picture of the icon at <paramref name="index"/>, in the list that keeps it at a
    /// size, or nothing when the shell will not give it.
    /// </summary>
    /// <remarks>
    /// The four lists hold the same icons, one size each — 16, 32, 48 and 256 pixels, measured rather
    /// than remembered — so the index one is named by is the index in all of them. What is asked for
    /// is the smallest list that is not smaller than the size wanted, since a picture enlarged is a
    /// picture the shell could have given better, and the shell keeps nothing between 48 and 256: a
    /// size in between is read at 256 and drawn smaller, which is what the grid's larger zooms are.
    /// </remarks>
    /// <param name="index">Where the icon sits in the shell's own list of icons.</param>
    /// <param name="size">How many pixels wide and tall the icon is wanted.</param>
    private static Bitmap? Listed(int index, int size)
    {
        var iid = IIDImageList;
        var chosen = size <= Small ? List.Small
            : size <= Large ? List.Large
            : size <= ExtraLarge ? List.ExtraLarge
            : List.Jumbo;

        if (SHGetImageList(chosen, ref iid, out var list) != 0 || list is null)
        {
            return null;
        }

        try
        {
            if (list.GetIcon(index, Transparent, out var icon) != 0 || icon == IntPtr.Zero)
            {
                return null;
            }

            try
            {
                return Pixels(icon);
            }
            finally
            {
                DestroyIcon(icon);
            }
        }
        finally
        {
            _ = Marshal.ReleaseComObject(list);
        }
    }

    /// <summary>The pixels of an icon handle, as a picture Avalonia can draw.</summary>
    private static Bitmap? Pixels(IntPtr icon)
    {
        if (!GetIconInfo(icon, out var parts))
        {
            return null;
        }

        try
        {
            var shape = default(BitmapShape);

            if (
                GetObjectW(parts.Colour, Marshal.SizeOf<BitmapShape>(), ref shape) == 0
                || shape.Width <= 0
                || shape.Height <= 0
                || shape.BitsPerPixel != 32
            )
            {
                // Anything but a 32-bit icon keeps its transparency in a mask rather than in an alpha
                // channel, which is a picture this deliberately does not assemble: the shell's own
                // icons have been 32-bit for as long as they have had transparency.
                return null;
            }

            var pixels = new byte[shape.Width * shape.Height * 4];
            var header = new BitmapInfo
            {
                Header = new BitmapInfoHeader
                {
                    Size = (uint)Marshal.SizeOf<BitmapInfoHeader>(),
                    Width = shape.Width,

                    // Negative, which is how a bitmap says its rows run top-down, the way a picture is
                    // held and the way Avalonia reads one.
                    Height = -shape.Height,
                    Planes = 1,
                    BitsPerPixel = 32,
                    Compression = Rgb,
                },
            };

            var dc = GetDC(IntPtr.Zero);

            if (dc == IntPtr.Zero)
            {
                return null;
            }

            try
            {
                if (GetDIBits(dc, parts.Colour, 0, (uint)shape.Height, pixels, ref header, 0) == 0)
                {
                    return null;
                }
            }
            finally
            {
                _ = ReleaseDC(IntPtr.Zero, dc);
            }

            return Picture(shape.Width, shape.Height, pixels);
        }
        finally
        {
            _ = DeleteObject(parts.Colour);
            _ = DeleteObject(parts.Mask);
        }
    }

    /// <summary>Copies pixels out of a device-independent bitmap into a picture.</summary>
    private static WriteableBitmap Picture(int width, int height, byte[] pixels)
    {
        var picture = new WriteableBitmap(
            new PixelSize(width, height),
            new Vector(96, 96),
            PixelFormat.Bgra8888,
            AlphaFormat.Unpremul
        );

        using (var locked = picture.Lock())
        {
            for (var row = 0; row < height; row++)
            {
                Marshal.Copy(
                    pixels,
                    row * width * 4,
                    IntPtr.Add(locked.Address, row * locked.RowBytes),
                    width * 4
                );
            }
        }

        return picture;
    }

    /// <summary>Asks the shell for an icon, or for what it knows about a name.</summary>
    [DllImport("shell32", CharSet = CharSet.Unicode)]
    private static extern IntPtr SHGetFileInfo(
        string path,
        uint attributes,
        ref SHFILEINFO info,
        uint size,
        uint flags
    );

    /// <summary>Asks the shell for one of the four lists it keeps its icons in.</summary>
    [DllImport("shell32")]
    private static extern int SHGetImageList(List list, ref Guid iid, out IImageList? images);

    /// <summary>Gives an icon handle back, since what the shell handed out is the caller's to free.</summary>
    [DllImport("user32")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool DestroyIcon(IntPtr icon);

    /// <summary>Where an icon keeps its two bitmaps: the picture, and the mask behind it.</summary>
    [DllImport("user32")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool GetIconInfo(IntPtr icon, out IconInfo info);

    /// <summary>What a bitmap handle is, which is how its pixels are asked for.</summary>
    [DllImport("gdi32", EntryPoint = "GetObjectW")]
    private static extern int GetObjectW(IntPtr handle, int size, ref BitmapShape shape);

    /// <summary>A device context, which asking a bitmap for its pixels needs one of.</summary>
    [DllImport("user32")]
    private static extern IntPtr GetDC(IntPtr window);

    /// <summary>Gives a device context back.</summary>
    [DllImport("user32")]
    private static extern int ReleaseDC(IntPtr window, IntPtr dc);

    /// <summary>Reads a bitmap's pixels, at a format asked for rather than the one it keeps.</summary>
    [DllImport("gdi32")]
    private static extern int GetDIBits(
        IntPtr dc,
        IntPtr bitmap,
        uint start,
        uint lines,
        byte[] pixels,
        ref BitmapInfo info,
        uint usage
    );

    /// <summary>Frees a bitmap the shell handed out.</summary>
    [DllImport("gdi32")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool DeleteObject(IntPtr handle);

    /// <summary>An icon, wanted as one rather than as what it stands for.</summary>
    private const uint Icon = 0x0000_0100;

    /// <summary>
    /// The place the icon has in the shell's own lists, which is what the icon is read from when it is
    /// wanted at a size the shell keeps a separate list for.
    /// </summary>
    private const uint SysIconIndex = 0x0000_4000;

    /// <summary>An answer about a kind of thing rather than about a path, so that nothing has to exist.</summary>
    private const uint UseFileAttributes = 0x0000_0010;

    /// <summary>What a directory is, as an attribute.</summary>
    private const uint FileAttributeDirectory = 0x0000_0010;

    /// <summary>What a file that is nothing in particular is, as an attribute.</summary>
    private const uint FileAttributeNormal = 0x0000_0080;

    /// <summary>What an icon drawn from a list is taken with: the mask it is cut out by is not drawn.</summary>
    private const uint Transparent = 0x0000_0001;

    /// <summary>Pixels packed four bytes to a pixel, in the order red, green, blue, unused.</summary>
    private const uint Rgb = 0;

    /// <summary>The size of the smallest picture the shell keeps of an icon.</summary>
    private const int Small = 16;

    /// <summary>The size of the second smallest, which is the one a path asked about is answered with.</summary>
    private const int Large = 32;

    /// <summary>The largest size below the one the shell keeps for a thumbnail view.</summary>
    private const int ExtraLarge = 48;

    /// <summary>The interface an icon list is reached through, which is the one the shell hands back.</summary>
    private static readonly Guid IIDImageList = new("46EB5926-582E-4017-9FDF-E8998DAA0950");

    /// <summary>
    /// One of the four lists the shell keeps its icons in, by the size it holds them at.
    /// </summary>
    /// <remarks>
    /// The values are the shell's own, and they are not in size order: the large list is the first one
    /// and the small one the second. What each actually holds — 32, 16, 48 and 256 — is what asking it
    /// answers with, which is not something the names say.
    /// </remarks>
    private enum List
    {
        /// <summary>Thirty-two pixels.</summary>
        Large = 0,

        /// <summary>Sixteen pixels.</summary>
        Small = 1,

        /// <summary>Forty-eight pixels.</summary>
        ExtraLarge = 2,

        /// <summary>Two hundred and fifty-six pixels, which is the largest the shell keeps.</summary>
        Jumbo = 4,
    }

    /// <summary>
    /// A list of icons, as the shell hands one over.
    /// </summary>
    /// <remarks>
    /// A COM interface is reached by the order its methods are declared in rather than by their names, so
    /// the seven that come before the one this needs are declared and never called: leaving them out would
    /// put <see cref="GetIcon"/> in front of them, where the shell's <c>AddMasked</c> is.
    /// </remarks>
    [ComImport]
    [Guid("46EB5926-582E-4017-9FDF-E8998DAA0950")]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    private interface IImageList
    {
        /// <summary>Never called: the first slot, which the shell's <c>Add</c> is at.</summary>
        [PreserveSig]
        int Add(IntPtr parameters);

        /// <summary>Never called: the second slot, which the shell's <c>ReplaceIcon</c> is at.</summary>
        [PreserveSig]
        int ReplaceIcon(int index, IntPtr icon, out int replaced);

        /// <summary>Never called: the third slot, which the shell's <c>SetOverlayImage</c> is at.</summary>
        [PreserveSig]
        int SetOverlayImage(int image, int overlay);

        /// <summary>Never called: the fourth slot, which the shell's <c>Replace</c> is at.</summary>
        [PreserveSig]
        int Replace(int index, IntPtr icon);

        /// <summary>Never called: the fifth slot, which the shell's <c>AddMasked</c> is at.</summary>
        [PreserveSig]
        int AddMasked(IntPtr mask, int colour);

        /// <summary>Never called: the sixth slot, which the shell's <c>Draw</c> is at.</summary>
        [PreserveSig]
        int Draw(IntPtr parameters);

        /// <summary>Never called: the seventh slot, which the shell's <c>Remove</c> is at.</summary>
        [PreserveSig]
        int Remove(int index);

        /// <summary>The icon at a place in this list, which the caller frees.</summary>
        /// <param name="index">Where the icon sits in the list.</param>
        /// <param name="flags">What the icon is taken with.</param>
        /// <param name="icon">The icon, or nothing where there is none.</param>
        [PreserveSig]
        int GetIcon(int index, uint flags, out IntPtr icon);
    }

    /// <summary>What the shell says about a name.</summary>
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct SHFILEINFO
    {
        /// <summary>The icon the shell drew, which the caller frees.</summary>
        public IntPtr Icon;

        /// <summary>Where the icon sits in the system's own list of them.</summary>
        public int Index;

        /// <summary>What the shell knows about the name, as attributes.</summary>
        public uint Attributes;

        /// <summary>The name the shell was given, as it would show it.</summary>
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 260)]
        public string Name;

        /// <summary>The name as a user would read it.</summary>
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 80)]
        public string DisplayName;

        /// <summary>What kind of thing the shell says it is, in words.</summary>
        public uint Type;
    }

    /// <summary>An icon's two bitmaps.</summary>
    [StructLayout(LayoutKind.Sequential)]
    private struct IconInfo
    {
        /// <summary>Whether the icon is one colour, which is what an icon drawn from a mask is.</summary>
        public int Monochrome;

        /// <summary>Where the cursor is, for an icon used as one.</summary>
        public int HotspotX;

        /// <summary>Where the cursor is, for an icon used as one.</summary>
        public int HotspotY;

        /// <summary>The mask, which is what an icon without an alpha channel is cut out by.</summary>
        public IntPtr Mask;

        /// <summary>The picture itself.</summary>
        public IntPtr Colour;
    }

    /// <summary>A bitmap's shape, as the system describes it.</summary>
    [StructLayout(LayoutKind.Sequential)]
    private struct BitmapShape
    {
        /// <summary>What kind of bitmap it is.</summary>
        public int Type;

        /// <summary>How many pixels wide it is.</summary>
        public int Width;

        /// <summary>How many pixels tall it is.</summary>
        public int Height;

        /// <summary>How many bytes one row takes, rounded up to a whole number of them.</summary>
        public int WidthBytes;

        /// <summary>How many planes it has, which is always one.</summary>
        public ushort Planes;

        /// <summary>How many bits one pixel takes.</summary>
        public ushort BitsPerPixel;

        /// <summary>Where its pixels are, or nothing when they are to be read another way.</summary>
        public IntPtr Bits;
    }

    /// <summary>How pixels are to be read out of a bitmap.</summary>
    [StructLayout(LayoutKind.Sequential)]
    private struct BitmapInfoHeader
    {
        /// <summary>How large this header is, which is what says which header it is.</summary>
        public uint Size;

        /// <summary>How many pixels wide the picture is.</summary>
        public int Width;

        /// <summary>How many pixels tall it is, negative for rows that run top-down.</summary>
        public int Height;

        /// <summary>How many planes the picture has.</summary>
        public ushort Planes;

        /// <summary>How many bits one pixel takes.</summary>
        public ushort BitsPerPixel;

        /// <summary>How the pixels are packed, which is not at all.</summary>
        public uint Compression;

        /// <summary>How many bytes the pixels take, which nothing here has to say.</summary>
        public uint SizeImage;

        /// <summary>How many pixels to a metre, which nothing here has to say.</summary>
        public int MetresX;

        /// <summary>How many pixels to a metre, which nothing here has to say.</summary>
        public int MetresY;

        /// <summary>How many colours are used, which a packed picture does not say.</summary>
        public uint ColoursUsed;

        /// <summary>How many colours matter, which a packed picture does not say.</summary>
        public uint ColoursImportant;
    }

    /// <summary>A header with the room the system expects to find one in.</summary>
    [StructLayout(LayoutKind.Sequential)]
    private struct BitmapInfo
    {
        /// <summary>How the pixels are to be read.</summary>
        public BitmapInfoHeader Header;

        /// <summary>Room for one colour, which a packed picture has none of.</summary>
        public uint Colour;
    }
}

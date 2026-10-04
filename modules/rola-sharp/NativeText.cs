using System.Runtime.InteropServices;

namespace RolaSharp;

/// <summary>
/// Reading the C strings the native side hands out.
/// </summary>
/// <remarks>
/// A string crosses as a buffer the caller owns, so reading one is two steps that must not come
/// apart: copy it into managed memory, and release the buffer. Doing both in one place is what keeps
/// a caller from reading one and forgetting the other.
/// </remarks>
internal static class NativeText
{
    /// <summary>Reads a C string and releases the buffer it arrived in.</summary>
    /// <param name="text">The buffer, or null when there is no string.</param>
    /// <returns>What it said, or an empty string.</returns>
    public static string Taken(nint text)
    {
        if (text == 0)
        {
            return string.Empty;
        }

        try
        {
            return Marshal.PtrToStringUTF8(text) ?? string.Empty;
        }
        finally
        {
            RorolalaBinding.free_string(text);
        }
    }
}

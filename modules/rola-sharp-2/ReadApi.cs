using System.Runtime.InteropServices;

using RolaSharp.V2.Write;

namespace RolaSharp.V2;

/// <summary>One read named by a JSON request, with a cancellation the caller may set.</summary>
public static class ReadApi
{
    /// <summary>Runs one read.</summary>
    /// <param name="directory">The directory the request's relative paths are read against.</param>
    /// <param name="request">The request, as the C ABI's aggregate read names it.</param>
    /// <param name="cancellation">The cancellation, or nothing for one nobody sets.</param>
    /// <returns>The document, or nothing when the read has no document.</returns>
    public static string? Json(
        string directory,
        string request,
        Cancellation? cancellation = null
    )
    {
        ArgumentNullException.ThrowIfNull(directory);
        ArgumentNullException.ThrowIfNull(request);

        var owned = cancellation is null ? Cancellation.New() : null;
        var token = cancellation ?? owned!;
        var directoryText = Utf8(directory);
        var requestText = Utf8(request);
        try
        {
            var answer = RorolalaBinding.rola_v2_read_json(
                directoryText,
                requestText,
                token.Handle
            );
            if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
            {
                throw ReadResults.V2(answer.payload);
            }

            var json = NativeText.Taken(answer.payload);

            return json is "" or "null" ? null : json;
        }
        finally
        {
            owned?.Dispose();
            Marshal.FreeCoTaskMem(directoryText);
            Marshal.FreeCoTaskMem(requestText);
        }
    }

    /// <summary>Runs one read on the thread pool, stopping it when the caller asks.</summary>
    /// <param name="directory">The directory the request's relative paths are read against.</param>
    /// <param name="request">The request, as the C ABI's aggregate read names it.</param>
    /// <param name="cancellationToken">The caller's token.</param>
    /// <returns>The document, or nothing when the read has no document.</returns>
    public static Task<string?> JsonAsync(
        string directory,
        string request,
        CancellationToken cancellationToken = default
    ) =>
        Task.Run(
            () =>
            {
                using var cancellation = Cancellation.New();
                using var registration = cancellationToken.Register(cancellation.Cancel);

                return Json(directory, request, cancellation);
            },
            CancellationToken.None
        );

    private static nint Utf8(string text) => Marshal.StringToCoTaskMemUTF8(text);
}

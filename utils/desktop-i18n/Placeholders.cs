using System.Globalization;
using System.Text;

namespace RorolalaDesktop.I18n;

/// <summary>
/// The places a written form leaves for values, and the values that go in them.
/// </summary>
/// <remarks>
/// A form leaves a place as <c>%{name}</c>, the way <c>rust-i18n</c> reads it. The name is there
/// for a reader of the file and is not matched against anything: values go in by order, the first
/// value into the first place, so what a form is called is not something a caller has to know.
/// </remarks>
internal static class Placeholders
{
    /// <summary>What a place is opened with.</summary>
    private const string Opening = "%{";

    /// <summary>What a place is closed with.</summary>
    private const char Closing = '}';

    /// <summary>
    /// The written form with the values put in the places it leaves, in order.
    /// </summary>
    /// <remarks>
    /// A place with no value left for it is left as it was written rather than filled with the
    /// nothing after it, so what a reader sees is what the file says. A value with no place left
    /// for it is dropped for the same reason: the form is what says how many there are.
    /// </remarks>
    public static string Fill(string form, object?[] values)
    {
        if (values.Length == 0)
        {
            return form;
        }

        var filled = new StringBuilder(form.Length);
        var at = 0;
        var next = 0;

        while (true)
        {
            var start = form.IndexOf(Opening, at, StringComparison.Ordinal);
            var end = start < 0 ? -1 : form.IndexOf(Closing, start + Opening.Length);

            // No place left to fill: what remains is written out as it stands and this is done.
            if (start < 0 || end < 0)
            {
                filled.Append(form, at, form.Length - at);
                return filled.ToString();
            }

            filled.Append(form, at, start - at);

            if (next < values.Length)
            {
                filled.Append(Value(values[next]));
                next += 1;
            }
            else
            {
                filled.Append(form, start, end - start + 1);
            }

            at = end + 1;
        }
    }

    /// <summary>
    /// A value as it is written into a form, the same in every language.
    /// </summary>
    /// <remarks>
    /// Values are written the way <c>rust-i18n</c> writes them, which is the way a number is a
    /// number: the machine's own language is not consulted, so what a form says does not change
    /// with where it is read.
    /// </remarks>
    private static string Value(object? value) =>
        Convert.ToString(value, CultureInfo.InvariantCulture) ?? string.Empty;
}

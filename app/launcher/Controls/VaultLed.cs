using Avalonia;
using Avalonia.Controls;
using Avalonia.Media;
using Avalonia.Threading;
using RorolalaLauncher.Theming;

namespace RorolalaLauncher.Controls;

/// <summary>
/// The lamp beside an address: what the last handshake with that Vault came to.
/// </summary>
/// <remarks>
/// The lamp is the state and nothing else: a filled green when the daemon answered, a red when it did not,
/// a hollow ring before a handshake has been spoken, and an amber that pulses while one is being spoken.
/// The pulse is the only motion in the dock, and it is there because a check that takes a moment should not
/// look like one that never started.
/// </remarks>
internal sealed class VaultLed : Border
{
    /// <summary>How often the amber opens and closes while a handshake is being spoken.</summary>
    private static readonly TimeSpan Beat = TimeSpan.FromMilliseconds(550);

    /// <summary>The beat the amber follows.</summary>
    private readonly DispatcherTimer _pulse;

    /// <summary>Makes a lamp of one size.</summary>
    /// <param name="extent">How wide and tall it is drawn.</param>
    public VaultLed(double extent = 9)
    {
        Width = extent;
        Height = extent;
        CornerRadius = new CornerRadius(extent / 2);
        HorizontalAlignment = Avalonia.Layout.HorizontalAlignment.Center;
        VerticalAlignment = Avalonia.Layout.VerticalAlignment.Center;

        _pulse = new DispatcherTimer { Interval = Beat };

        _pulse.Tick += (_, _) => Opacity = Opacity >= 1 ? 0.42 : 1;
    }

    /// <summary>Stops the pulse, which is what a lamp taken out of the tree has to do.</summary>
    public void Stop() => _pulse.Stop();

    /// <summary>Draws the lamp for a state.</summary>
    /// <param name="status">What the last handshake came to.</param>
    /// <param name="skin">The colours in force.</param>
    public void Show(VaultStatus status, Skin skin)
    {
        ArgumentNullException.ThrowIfNull(skin);

        _pulse.Stop();

        Opacity = 1;
        BorderThickness = default;
        BorderBrush = null;
        Background = Brushes.Transparent;

        switch (status)
        {
            case VaultStatus.Reachable:
                Background = skin.Success;

                break;

            case VaultStatus.Failed:
                Background = skin.Alarm;

                break;

            case VaultStatus.Checking:
                Background = skin.Warning;

                _pulse.Start();

                break;

            default:
                BorderThickness = new Thickness(1);
                BorderBrush = skin.Faint;

                break;
        }
    }
}

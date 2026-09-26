using RorolalaDesktop.Configuration;
using RorolalaDesktop.Docking;
using RorolalaDesktop.Hosting;
using RorolalaDesktop.I18n;
using RorolalaDesktop.Logging;
using RorolalaDesktop.Theming;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// A host's worth of shared services, for a test that starts plugins or opens docks.
/// </summary>
/// <remarks>
/// The services are the host's own, over scratch state: nothing here is a stand-in, and nothing
/// touches the person's own configuration or translations.
/// </remarks>
internal static class Host
{
    /// <summary>Makes the services a plugin is handed at initialization.</summary>
    /// <returns>Everything the host holds.</returns>
    public static HostServices Services()
    {
        // A directory of nothing is enough: a key nothing states reads as the key itself.
        RolaI18N.SetTranslationDirectory(Scratch.New("i18n"));

        var notifications = new NotificationService();
        var i18n = new I18nService();
        var log = new LogService(notifications);
        var popups = new PopupService(notifications);
        var preference = new PreferenceConfiguration();

        // The shell is made before the object it is put in, because a dialog is shown on the window the shell
        // names, which is the one place both sides agree on.
        var shell = new Shell();
        var dialogs = new Dialogs(shell, i18n);

        return new HostServices
        {
            Log = log,
            Popups = popups,
            I18n = i18n,
            // There is no application here, so the look is never applied; the service is what the
            // preference panel reads and writes, which is all a headless test has any use for.
            Theme = new ThemeService(null, new ThemeConfiguration()),
            Rola = new RolaCapability(),
            Preference = preference,
            Settings = new SettingRegistry(preference),
            Shell = shell,
            Menu = new MenuRegistry(),
            ContextMenus = new ContextMenuRegistry(),
            Navigation = new NavigationRegistry(),
            Docks = new DockManager(new DockRegistry(), i18n, log, popups),
            OpenHooks = new OpenHookRegistry(),
            IconBadges = new IconBadgeRegistry(),
            // Nothing activates a window in a headless test, so nothing is ever raised on it.
            Refocus = new Refocus(),
            // A dialog needs a window to be shown on, and a headless test has none: nothing is ever shown.
            Dialogs = dialogs,
        };
    }
}

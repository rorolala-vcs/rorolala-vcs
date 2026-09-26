using RorolalaDesktop.Configuration;
using RorolalaDesktop.Docking;
using RorolalaDesktop.Hosting;
using RorolalaDesktop.I18n;
using RorolalaDesktop.Logging;

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

        return new HostServices
        {
            Log = log,
            Popups = popups,
            I18n = i18n,
            Rola = new RolaCapability(),
            Preference = preference,
            Settings = new SettingRegistry(preference),
            Shell = new Shell(),
            Menu = new MenuRegistry(),
            ContextMenus = new ContextMenuRegistry(),
            Navigation = new NavigationRegistry(),
            Docks = new DockManager(new DockRegistry(), i18n, log, popups),
            OpenHooks = new OpenHookRegistry(),
            IconBadges = new IconBadgeRegistry(),
        };
    }
}

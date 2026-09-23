using Avalonia;
using Avalonia.Controls;
using RorolalaDesktop.I18n;

namespace RorolalaDesktop;

public partial class MainWindow : Window
{
    public MainWindow()
    {
        InitializeComponent();

        // The window is named where every other word is, so changing what it is called is a change
        // to a translation rather than to this file. A window built with no translations to read —
        // which is what the designer builds — keeps the name the markup gives it.
        if (RolaI18N.TranslationDirectory is not null)
        {
            Title = RolaI18N.Get("window.title");
        }

#if DEBUG
        this.AttachDevTools();
#endif
    }
}

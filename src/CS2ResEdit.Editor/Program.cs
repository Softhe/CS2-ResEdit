namespace Softhe.CS2ResEdit.Editor;

internal static class Program
{
    [STAThread]
    private static int Main(string[] args)
    {
        Application.SetHighDpiMode(HighDpiMode.PerMonitorV2);
        ApplicationConfiguration.Initialize();
        if (args.Length != 0)
        {
            MessageBox.Show(
                "Command-line options are not supported in this release. Start the application without arguments.",
                "Unsupported command",
                MessageBoxButtons.OK,
                MessageBoxIcon.Error);
            return 2;
        }

        var settingsPath = Environment.GetEnvironmentVariable("CS2_RESEDIT_SETTINGS_PATH");
        var steamRoot = Environment.GetEnvironmentVariable("CS2_RESEDIT_STEAM_ROOT");
        Application.Run(new MainForm(settingsPath, steamRoot));
        return 0;
    }
}

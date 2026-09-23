namespace Softhe.CS2ResEdit.Editor;

internal static class Program
{
    private const string SingleInstanceMutexName = @"Global\Softhe.CS2ResEdit.v1";

    [STAThread]
    private static int Main(string[] args)
    {
        if (args.Any(a => a is "--help" or "-h" or "-?" or "/?"))
        {
            MessageBox.Show(
                "CS2 ResEdit — safe local editor for Counter-Strike 2 display settings.\n\nUsage: CS2-ResEdit.exe [--help] [--version]\n\nThis release is GUI-only; start it without arguments to open the editor.",
                "CS2 ResEdit usage",
                MessageBoxButtons.OK,
                MessageBoxIcon.Information);
            return 0;
        }

        if (args.Any(a => a is "--version" or "-v"))
        {
            var version = typeof(Program).Assembly.GetName().Version?.ToString() ?? "unknown";
            MessageBox.Show($"CS2 ResEdit {version}", "CS2 ResEdit version",
                MessageBoxButtons.OK, MessageBoxIcon.Information);
            return 0;
        }

        if (args.Length != 0)
        {
            MessageBox.Show(
                "Command-line options are not supported in this release. Start the application without arguments (--help for usage).",
                "Unsupported command",
                MessageBoxButtons.OK,
                MessageBoxIcon.Error);
            return 2;
        }

        using var mutex = new Mutex(true, SingleInstanceMutexName, out var createdNew);
        if (!createdNew)
        {
            MessageBox.Show(
                "Another instance of CS2 ResEdit is already running.",
                "Already running",
                MessageBoxButtons.OK,
                MessageBoxIcon.Information);
            return 1;
        }

        Application.SetHighDpiMode(HighDpiMode.PerMonitorV2);
        ApplicationConfiguration.Initialize();

        var settingsPath = Environment.GetEnvironmentVariable("CS2_RESEDIT_SETTINGS_PATH");
        var steamRoot = Environment.GetEnvironmentVariable("CS2_RESEDIT_STEAM_ROOT");
        Application.Run(new MainForm(settingsPath, steamRoot));
        return 0;
    }
}

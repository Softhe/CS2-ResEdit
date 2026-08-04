using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;

namespace Softhe.CS2ResEdit.Core;

public sealed class DiagnosticService(VideoConfigService configs)
{
    private static readonly JsonSerializerOptions JsonOptions = new() { WriteIndented = true };

    public DiagnosticReport Create(
        string applicationVersion,
        IReadOnlyList<DisplayInfo> displays,
        int steamRootCount,
        int accountCount,
        string? configPath)
    {
        ConfigurationInspection? inspection = null;
        string status = configPath is null ? "Not selected" : "Valid";
        string? error = null;
        if (configPath is not null)
        {
            try { inspection = configs.Inspect(configPath); }
            catch (Exception ex) { status = "Invalid"; error = ex.GetType().Name; }
        }

        return new DiagnosticReport(
            1,
            DateTime.UtcNow,
            applicationVersion,
            RuntimeInformation.OSDescription,
            RuntimeInformation.ProcessArchitecture.ToString(),
            displays.Select(x => new DiagnosticDisplay(x.IsPrimary, x.CurrentWidth, x.CurrentHeight, x.Modes.Count)).ToArray(),
            steamRootCount,
            accountCount,
            status,
            inspection?.Encoding,
            inspection?.HasBom,
            inspection?.LineEnding,
            error);
    }

    public static string ToJson(DiagnosticReport report) => JsonSerializer.Serialize(report, JsonOptions);

    public static string ToSummary(DiagnosticReport report)
    {
        var text = new StringBuilder();
        text.AppendLine($"CS2 ResEdit {report.ApplicationVersion}");
        text.AppendLine($"OS: {report.OperatingSystem}");
        text.AppendLine($"Architecture: {report.Architecture}");
        text.AppendLine($"Displays: {report.Displays.Count} ({report.Displays.Sum(x => x.ModeCount)} reported modes)");
        text.AppendLine($"Steam roots: {report.SteamRootCount}");
        text.AppendLine($"Accounts discovered: {report.AccountCount}");
        text.AppendLine($"Configuration: {report.ConfigurationStatus}");
        if (report.ConfigurationEncoding is not null)
            text.AppendLine($"Encoding: {report.ConfigurationEncoding}; BOM: {report.ConfigurationHasBom}; lines: {report.ConfigurationLineEnding}");
        if (report.ErrorCategory is not null) text.AppendLine($"Error category: {report.ErrorCategory}");
        text.Append("No Steam names, identifiers, paths, configuration contents, or preferences are included.");
        return text.ToString();
    }
}

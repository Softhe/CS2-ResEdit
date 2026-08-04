using Softhe.CS2ResEdit.Core;

namespace Softhe.CS2ResEdit.Tests;

public sealed class DisplayAndDiagnosticsTests : IDisposable
{
    private readonly string root = Path.Combine(Path.GetTempPath(), "cs2-diagnostics-tests-" + Guid.NewGuid().ToString("N"));

    public DisplayAndDiagnosticsTests() => Directory.CreateDirectory(root);

    [Fact]
    public void DiagnosticReportIsVersionedAndPrivacySafe()
    {
        var path = Path.Combine(root, "personal-account-76561198000000000", "cs2_video.txt");
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        File.WriteAllText(path, "\"setting.defaultres\" \"1920\"\r\n\"setting.defaultresheight\" \"1080\"\r\n\"setting.aspectratiomode\" \"1\"\r\n");
        var displays = new[]
        {
            new DisplayInfo("private-device-name", "Personal Monitor", true, 2560, 1440,
                [new DisplayResolution(1920, 1080), new DisplayResolution(2560, 1440)])
        };

        var service = new DiagnosticService(new VideoConfigService());
        var report = service.Create("1.0.0", displays, 1, 2, path);
        var json = DiagnosticService.ToJson(report);
        var summary = DiagnosticService.ToSummary(report);

        Assert.Equal(1, report.SchemaVersion);
        Assert.Equal("Valid", report.ConfigurationStatus);
        Assert.Equal("CRLF", report.ConfigurationLineEnding);
        Assert.Contains("1.0.0", summary);
        Assert.DoesNotContain(path, json, StringComparison.OrdinalIgnoreCase);
        Assert.DoesNotContain("personal-account", json, StringComparison.OrdinalIgnoreCase);
        Assert.DoesNotContain("76561198000000000", json, StringComparison.Ordinal);
        Assert.DoesNotContain("Personal Monitor", json, StringComparison.Ordinal);
        Assert.DoesNotContain("private-device-name", json, StringComparison.Ordinal);
        Assert.DoesNotContain("setting.defaultres", json, StringComparison.Ordinal);
    }

    [Fact]
    public void DiagnosticReportSanitizesConfigurationFailure()
    {
        var path = Path.Combine(root, "secret-path.txt");
        File.WriteAllText(path, "invalid");
        var report = new DiagnosticService(new VideoConfigService()).Create("1.0.0", [], 0, 0, path);
        var json = DiagnosticService.ToJson(report);

        Assert.Equal("Invalid", report.ConfigurationStatus);
        Assert.Equal(nameof(InvalidDataException), report.ErrorCategory);
        Assert.DoesNotContain(path, json, StringComparison.OrdinalIgnoreCase);
    }

    [Fact]
    public void DisplayModelsExposeNeutralLabelsAndUniqueModes()
    {
        var display = new DisplayInfo("DISPLAY1", "Monitor", true, 1920, 1080,
            [new DisplayResolution(1280, 720), new DisplayResolution(1920, 1080)]);
        Assert.Contains("Primary", display.Label);
        Assert.Equal("1920x1080", display.Modes[1].Display);
    }

    public void Dispose()
    {
        Directory.Delete(root, true);
        GC.SuppressFinalize(this);
    }
}

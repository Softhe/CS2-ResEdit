namespace Softhe.CS2ResEdit.Core;

public sealed record Resolution(int Width, int Height, string Ratio, int Mode)
{
    public string Display => $"{Width}x{Height}";
}

public sealed record VideoConfigState(int Width, int Height, int AspectMode);

public sealed record SteamAccount(
    string DisplayName,
    string AccountId,
    ulong SteamId64,
    string PersonaName,
    string? AccountName,
    bool MostRecent,
    string ConfigPath,
    bool HasConfig,
    DateTime LastWriteTime);

public sealed record BackupInfo(string Path, DateTime Created, VideoConfigState State);

public sealed class Preferences
{
    public int SchemaVersion { get; set; } = 1;
    public string? LastAccountId { get; set; }
    public List<string> RecentConfigPaths { get; set; } = [];
    public string? Warning { get; set; }
}

public sealed record UpdateResult(bool Changed, string? BackupPath);

public sealed record DisplayResolution(int Width, int Height)
{
    public string Display => $"{Width}x{Height}";
}

public sealed record DisplayInfo(
    string DeviceName,
    string FriendlyName,
    bool IsPrimary,
    int CurrentWidth,
    int CurrentHeight,
    IReadOnlyList<DisplayResolution> Modes)
{
    public string Label => $"{FriendlyName}  ·  {CurrentWidth}x{CurrentHeight}{(IsPrimary ? "  ·  Primary" : "")}";
    public override string ToString() => Label;
}

public sealed record ConfigurationInspection(
    VideoConfigState State,
    string Encoding,
    bool HasBom,
    string LineEnding);

public sealed record DiagnosticDisplay(
    bool IsPrimary,
    int CurrentWidth,
    int CurrentHeight,
    int ModeCount);

public sealed record DiagnosticReport(
    int SchemaVersion,
    DateTime GeneratedUtc,
    string ApplicationVersion,
    string OperatingSystem,
    string Architecture,
    IReadOnlyList<DiagnosticDisplay> Displays,
    int SteamRootCount,
    int AccountCount,
    string ConfigurationStatus,
    string? ConfigurationEncoding,
    bool? ConfigurationHasBom,
    string? ConfigurationLineEnding,
    string? ErrorCategory);

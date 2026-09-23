using System.Text.Json;

namespace Softhe.CS2ResEdit.Core;

public sealed class PreferencesService(VideoConfigService configs)
{
    private static readonly JsonSerializerOptions JsonOptions = new() { WriteIndented = true };

    public static string DefaultPath => Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
        "Softhe", "CS2-ResEdit", "v1", "settings.json");

    public Preferences Read(string? path = null)
    {
        path ??= DefaultPath;
        var result = new Preferences();
        if (!File.Exists(path)) return result;
        try
        {
            if (new FileInfo(path) is { Exists: true } info && info.Length > 1024 * 1024)
                throw new InvalidDataException($"Settings file is too large ({info.Length} bytes).");
            var parsed = JsonSerializer.Deserialize<Preferences>(File.ReadAllText(path),
                new JsonSerializerOptions { PropertyNameCaseInsensitive = true })
                ?? throw new InvalidDataException("Settings are empty.");
            if (parsed.SchemaVersion is not (1 or 2)) throw new InvalidDataException($"Unsupported preference schema '{parsed.SchemaVersion}'.");
            result.SchemaVersion = 2;
            result.LastAccountId = parsed.LastAccountId;
            result.RecentConfigPaths = Clean(parsed.RecentConfigPaths);
            result.LastDisplayDevice = string.IsNullOrWhiteSpace(parsed.LastDisplayDevice) ? null : parsed.LastDisplayDevice;
            result.LastAspectMode = parsed.LastAspectMode is >= 0 and <= 2 ? parsed.LastAspectMode : null;
            result.WindowWidth = parsed.WindowWidth is >= 400 and <= 10000 ? parsed.WindowWidth : null;
            result.WindowHeight = parsed.WindowHeight is >= 300 and <= 10000 ? parsed.WindowHeight : null;
        }
        catch (Exception ex)
        {
            result.Warning = SanitizeWarning(ex);
        }
        return result;
    }

    internal static string SanitizeWarning(Exception ex) => ex switch
    {
        InvalidDataException => $"Preferences could not be loaded: {ex.Message}",
        System.Text.Json.JsonException => "Preferences could not be loaded: settings file is corrupt.",
        IOException => "Preferences could not be loaded: settings file is unreadable.",
        UnauthorizedAccessException => "Preferences could not be loaded: access to the settings file was denied.",
        ArgumentException => "Preferences could not be loaded: settings location is invalid.",
        NotSupportedException => "Preferences could not be loaded: settings location is invalid.",
        _ => "Preferences could not be loaded: unexpected error."
    };

    public Preferences Save(string? lastAccountId, IEnumerable<string> recentPaths, string? path = null,
        string? lastDisplayDevice = null, int? lastAspectMode = null, int? windowWidth = null, int? windowHeight = null)
    {
        path ??= DefaultPath;
        var result = new Preferences
        {
            SchemaVersion = 2,
            LastAccountId = string.IsNullOrWhiteSpace(lastAccountId) ? null : lastAccountId,
            RecentConfigPaths = Clean(recentPaths),
            LastDisplayDevice = string.IsNullOrWhiteSpace(lastDisplayDevice) ? null : lastDisplayDevice,
            LastAspectMode = lastAspectMode is >= 0 and <= 2 ? lastAspectMode : null,
            WindowWidth = windowWidth is >= 400 and <= 10000 ? windowWidth : null,
            WindowHeight = windowHeight is >= 300 and <= 10000 ? windowHeight : null
        };
        Directory.CreateDirectory(Path.GetDirectoryName(Path.GetFullPath(path))!);
        var temp = path + $".{Guid.NewGuid():N}.tmp";
        File.WriteAllText(temp, JsonSerializer.Serialize(result, JsonOptions), new System.Text.UTF8Encoding(false));
        try
        {
            if (File.Exists(path)) File.Replace(temp, path, null);
            else File.Move(temp, path);
        }
        finally { if (File.Exists(temp)) File.Delete(temp); }
        return result;
    }

    public List<string> AddRecent(IEnumerable<string> paths, string path)
    {
        var ordered = new List<string>();
        foreach (var candidate in new[] { path }.Concat(paths))
        {
            if (string.IsNullOrWhiteSpace(candidate)) continue;
            try { ordered.Add(Path.GetFullPath(candidate)); }
            catch (Exception) { }
        }
        return ordered.Distinct(StringComparer.OrdinalIgnoreCase).Take(5).ToList();
    }

    private List<string> Clean(IEnumerable<string>? paths)
    {
        var result = new List<string>();
        foreach (var candidate in paths ?? [])
        {
            if (string.IsNullOrWhiteSpace(candidate)) continue;
            try
            {
                var full = Path.GetFullPath(candidate);
                if (result.Contains(full, StringComparer.OrdinalIgnoreCase) || !File.Exists(full)) continue;
                _ = configs.Read(full);
                result.Add(full);
                if (result.Count == 5) break;
            }
            catch (Exception) { }
        }
        return result;
    }
}

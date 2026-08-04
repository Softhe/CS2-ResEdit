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
            var parsed = JsonSerializer.Deserialize<Preferences>(File.ReadAllText(path),
                new JsonSerializerOptions { PropertyNameCaseInsensitive = true })
                ?? throw new InvalidDataException("Settings are empty.");
            if (parsed.SchemaVersion != 1) throw new InvalidDataException($"Unsupported preference schema '{parsed.SchemaVersion}'.");
            result.LastAccountId = parsed.LastAccountId;
            result.RecentConfigPaths = Clean(parsed.RecentConfigPaths);
        }
        catch (Exception ex)
        {
            result.Warning = $"Preferences could not be loaded: {ex.Message}";
        }
        return result;
    }

    public Preferences Save(string? lastAccountId, IEnumerable<string> recentPaths, string? path = null)
    {
        path ??= DefaultPath;
        var result = new Preferences
        {
            LastAccountId = string.IsNullOrWhiteSpace(lastAccountId) ? null : lastAccountId,
            RecentConfigPaths = Clean(recentPaths)
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

    public List<string> AddRecent(IEnumerable<string> paths, string path) =>
        new[] { path }.Concat(paths).Select(Path.GetFullPath)
            .Distinct(StringComparer.OrdinalIgnoreCase).Take(5).ToList();

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

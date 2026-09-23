using Microsoft.Win32;

namespace Softhe.CS2ResEdit.Core;

public sealed class SteamService
{
    public const ulong SteamIdBase = 76561197960265728UL;
    private const string ConfigRelativePath = @"730\local\cfg\cs2_video.txt";

    public static ulong ToSteamId64(uint accountId) => SteamIdBase + accountId;

    public IReadOnlyList<string> GetRoots(string? overrideRoot = null)
    {
        var candidates = new List<string>();
        if (NormalizeRoot(overrideRoot) is { } normalizedOverride) candidates.Add(normalizedOverride);
        AddRegistryRoot(candidates, Registry.CurrentUser, @"Software\Valve\Steam");
        AddRegistryRoot(candidates, Registry.LocalMachine, @"SOFTWARE\WOW6432Node\Valve\Steam");
        AddRegistryRoot(candidates, Registry.LocalMachine, @"SOFTWARE\Valve\Steam");
        try { candidates.Add(Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ProgramFilesX86), "Steam")); }
        catch (Exception) { }
        var roots = new List<string>();
        foreach (var candidate in candidates)
        {
            string full;
            try
            {
                if (!Directory.Exists(candidate)) continue;
                full = Path.GetFullPath(candidate);
            }
            catch (Exception) { continue; }
            if (!roots.Contains(full, StringComparer.OrdinalIgnoreCase)) roots.Add(full);
        }
        return roots;
    }

    /// <summary>Normalizes a user-supplied root without throwing. Returns null when unusable.</summary>
    public static string? NormalizeRoot(string? root)
    {
        if (string.IsNullOrWhiteSpace(root)) return null;
        try
        {
            var trimmed = Environment.ExpandEnvironmentVariables(root.Trim().Trim('"').Trim());
            if (string.IsNullOrWhiteSpace(trimmed)) return null;
            return Path.GetFullPath(trimmed);
        }
        catch (Exception)
        {
            return null;
        }
    }

    public IReadOnlyList<SteamAccount> GetAccounts(IEnumerable<string>? roots = null)
    {
        roots ??= GetRoots();
        var accounts = new Dictionary<string, SteamAccount>(StringComparer.OrdinalIgnoreCase);
        foreach (var root in roots)
        {
            IReadOnlyList<LoginUser> loginUsers;
            try { loginUsers = ParseLoginUsers(Path.Combine(root, "config", "loginusers.vdf")); }
            catch (Exception) { loginUsers = []; }
            var users = loginUsers.ToDictionary(x => x.SteamId64);
            IReadOnlyList<string> directories;
            try
            {
                var userdata = Path.Combine(root, "userdata");
                if (!Directory.Exists(userdata)) continue;
                directories = Directory.EnumerateDirectories(userdata).ToArray();
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or ArgumentException or NotSupportedException)
            {
                continue;
            }
            foreach (var directory in directories)
            {
                try
                {
                    if (!uint.TryParse(Path.GetFileName(directory), out var accountId)) continue;
                    var steamId = ToSteamId64(accountId);
                    users.TryGetValue(steamId, out var login);
                    var config = Path.Combine(directory, ConfigRelativePath);
                    var exists = File.Exists(config);
                    var persona = login?.PersonaName ?? $"Steam account {accountId}";
                    var display = $"{persona}  -  Account ID {accountId}  -  SteamID64 {steamId}";
                    if (!exists) display += "  (CS2 config not found)";
                    var candidate = new SteamAccount(display, accountId.ToString(), steamId, persona,
                        login?.AccountName, login?.MostRecent ?? false, config, exists,
                        exists ? File.GetLastWriteTime(config) : DateTime.MinValue);
                    if (accounts.TryGetValue(candidate.AccountId, out var current) && !Prefer(candidate, current))
                        continue;
                    accounts[candidate.AccountId] = candidate;
                }
                catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or ArgumentException or NotSupportedException)
                {
                    continue;
                }
            }
        }
        return accounts.Values.OrderByDescending(x => x.MostRecent)
            .ThenByDescending(x => x.LastWriteTime).ThenBy(x => x.PersonaName).ToArray();
    }

    private static bool Prefer(SteamAccount candidate, SteamAccount current)
    {
        if (candidate.HasConfig != current.HasConfig) return candidate.HasConfig;
        if (candidate.LastWriteTime != current.LastWriteTime) return candidate.LastWriteTime > current.LastWriteTime;
        if (candidate.MostRecent != current.MostRecent) return candidate.MostRecent;
        return false;
    }

    public IReadOnlyList<LoginUser> ParseLoginUsers(string path)
    {
        if (!File.Exists(path)) return [];
        try
        {
            if (new FileInfo(path) is { Exists: true } info && info.Length > 5 * 1024 * 1024) return [];
            var root = ValveKeyValues.Parse(File.ReadAllText(path));
            var users = root.GetObjects("users").FirstOrDefault();
            if (users is null) return [];
            return users.Entries.Where(x => x.Object is not null && ulong.TryParse(x.Name, out _))
                .Select(x => new LoginUser(
                    ulong.TryParse(x.Name, out var parsedId) ? parsedId : 0,
                    x.Object!.GetString("AccountName"),
                    x.Object.GetString("PersonaName") ?? "Unknown Steam account",
                    x.Object.GetString("MostRecent") == "1"))
                .GroupBy(x => x.SteamId64).Select(x => x.Last()).ToArray();
        }
        catch (IOException) { return []; }
        catch (UnauthorizedAccessException) { return []; }
        catch (InvalidDataException) { return []; }
        catch (ArgumentException) { return []; }
        catch (NotSupportedException) { return []; }
    }

    private static void AddRegistryRoot(List<string> roots, RegistryKey hive, string keyPath)
    {
        try
        {
            using var key = hive.OpenSubKey(keyPath);
            var path = key?.GetValue("SteamPath") as string ?? key?.GetValue("InstallPath") as string;
            if (!string.IsNullOrWhiteSpace(path)) roots.Add(path);
        }
        catch (Exception) { }
    }
}

public sealed record LoginUser(ulong SteamId64, string? AccountName, string PersonaName, bool MostRecent);

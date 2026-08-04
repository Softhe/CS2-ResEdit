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
        if (!string.IsNullOrWhiteSpace(overrideRoot)) candidates.Add(overrideRoot);
        AddRegistryRoot(candidates, Registry.CurrentUser, @"Software\Valve\Steam");
        AddRegistryRoot(candidates, Registry.LocalMachine, @"SOFTWARE\WOW6432Node\Valve\Steam");
        AddRegistryRoot(candidates, Registry.LocalMachine, @"SOFTWARE\Valve\Steam");
        candidates.Add(Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ProgramFilesX86), "Steam"));
        return candidates.Where(Directory.Exists).Select(Path.GetFullPath)
            .Distinct(StringComparer.OrdinalIgnoreCase).ToArray();
    }

    public IReadOnlyList<SteamAccount> GetAccounts(IEnumerable<string>? roots = null)
    {
        roots ??= GetRoots();
        var accounts = new Dictionary<string, SteamAccount>(StringComparer.OrdinalIgnoreCase);
        foreach (var root in roots)
        {
            var users = ParseLoginUsers(Path.Combine(root, "config", "loginusers.vdf"))
                .ToDictionary(x => x.SteamId64);
            var userdata = Path.Combine(root, "userdata");
            if (!Directory.Exists(userdata)) continue;
            foreach (var directory in Directory.EnumerateDirectories(userdata))
            {
                if (!uint.TryParse(Path.GetFileName(directory), out var accountId)) continue;
                var steamId = ToSteamId64(accountId);
                users.TryGetValue(steamId, out var login);
                var config = Path.Combine(directory, ConfigRelativePath);
                var exists = File.Exists(config);
                var persona = login?.PersonaName ?? $"Steam account {accountId}";
                var display = $"{persona}  -  Account ID {accountId}  -  SteamID64 {steamId}";
                if (!exists) display += "  (CS2 config not found)";
                accounts[config] = new SteamAccount(display, accountId.ToString(), steamId, persona,
                    login?.AccountName, login?.MostRecent ?? false, config, exists,
                    exists ? File.GetLastWriteTime(config) : DateTime.MinValue);
            }
        }
        return accounts.Values.OrderByDescending(x => x.MostRecent)
            .ThenByDescending(x => x.LastWriteTime).ThenBy(x => x.PersonaName).ToArray();
    }

    public IReadOnlyList<LoginUser> ParseLoginUsers(string path)
    {
        if (!File.Exists(path)) return [];
        try
        {
            var root = ValveKeyValues.Parse(File.ReadAllText(path));
            var users = root.GetObjects("users").FirstOrDefault();
            if (users is null) return [];
            return users.Entries.Where(x => x.Object is not null && ulong.TryParse(x.Name, out _))
                .Select(x => new LoginUser(
                    ulong.Parse(x.Name),
                    x.Object!.GetString("AccountName"),
                    x.Object.GetString("PersonaName") ?? "Unknown Steam account",
                    x.Object.GetString("MostRecent") == "1"))
                .GroupBy(x => x.SteamId64).Select(x => x.Last()).ToArray();
        }
        catch (IOException) { return []; }
        catch (UnauthorizedAccessException) { return []; }
        catch (InvalidDataException) { return []; }
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

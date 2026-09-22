using System.Text;
using System.Text.RegularExpressions;

namespace Softhe.CS2ResEdit.Core;

public sealed partial class VideoConfigService
{
    private const string WidthKey = "setting.defaultres";
    private const string HeightKey = "setting.defaultresheight";
    private const string ModeKey = "setting.aspectratiomode";

    public VideoConfigState Read(string path)
    {
        var document = TextDocument.Load(path);
        return Parse(document.Text);
    }

    public ConfigurationInspection Inspect(string path)
    {
        var document = TextDocument.Load(path);
        var state = Parse(document.Text);
        var lineEnding = document.Text.Contains("\r\n", StringComparison.Ordinal) ? "CRLF"
            : document.Text.Contains('\n') ? "LF" : "None";
        return new ConfigurationInspection(state, document.Encoding.WebName, document.HasBom, lineEnding);
    }

    public VideoConfigState Parse(string text) => new(
        ReadUnique(text, WidthKey),
        ReadUnique(text, HeightKey),
        ReadUnique(text, ModeKey, 0, 2));

    public UpdateResult Update(string path, Resolution resolution, bool createBackup = true)
    {
        var fullPath = Path.GetFullPath(path);
        var document = TextDocument.Load(fullPath);
        var current = Parse(document.Text);
        if (current == new VideoConfigState(resolution.Width, resolution.Height, resolution.Mode))
            return new UpdateResult(false, null);

        var changed = ReplaceUnique(document.Text, WidthKey, resolution.Width);
        changed = ReplaceUnique(changed, HeightKey, resolution.Height);
        changed = ReplaceUnique(changed, ModeKey, resolution.Mode);
        _ = Parse(changed);

        string? backupPath = null;
        if (createBackup)
        {
            backupPath = NewBackupPath(fullPath);
            File.Copy(fullPath, backupPath, false);
            try { _ = Read(backupPath); }
            catch
            {
                try { File.Delete(backupPath); } catch (Exception) { }
                throw;
            }
        }

        try
        {
            AtomicWrite(fullPath, document.Encode(changed));
            if (createBackup) RetainBackups(fullPath);
        }
        catch
        {
            if (backupPath is not null && File.Exists(backupPath))
            {
                ClearReadOnly(fullPath);
                File.Copy(backupPath, fullPath, true);
            }
            throw;
        }
        return new UpdateResult(true, backupPath);
    }

    public IReadOnlyList<BackupInfo> GetBackups(string configPath)
    {
        var fullPath = Path.GetFullPath(configPath);
        var directory = Path.GetDirectoryName(fullPath)!;
        if (!Directory.Exists(directory)) return [];
        var prefix = Path.GetFileName(fullPath) + ".";
        return Directory.EnumerateFiles(directory, prefix + "*.bak")
            .Where(p => BackupNameRegex(prefix).IsMatch(Path.GetFileName(p)))
            .Select(p =>
            {
                try { return new BackupInfo(p, File.GetLastWriteTime(p), Read(p)); }
                catch { return null; }
            })
            .OfType<BackupInfo>()
            .OrderByDescending(x => BackupOrderKey(Path.GetFileName(x.Path), prefix))
            .ThenByDescending(x => x.Created)
            .ToArray();
    }

    public string Restore(string configPath, string backupPath)
    {
        var fullConfig = Path.GetFullPath(configPath);
        var fullBackup = Path.GetFullPath(backupPath);
        _ = Read(fullConfig);
        _ = Read(fullBackup);
        if (!GetBackups(fullConfig).Any(x => string.Equals(x.Path, fullBackup, StringComparison.OrdinalIgnoreCase)))
            throw new InvalidOperationException("The selected file is not a recognized backup for this configuration.");

        var rollback = NewBackupPath(fullConfig);
        File.Copy(fullConfig, rollback, false);
        _ = Read(rollback);
        try { AtomicWrite(fullConfig, File.ReadAllBytes(fullBackup)); }
        catch
        {
            ClearReadOnly(fullConfig);
            File.Copy(rollback, fullConfig, true);
            throw;
        }
        RetainBackups(fullConfig);
        return rollback;
    }

    private static int ReadUnique(string text, string key, int min = 1, int max = 32768)
    {
        var matches = ValueRegex(key).Matches(text);
        if (matches.Count != 1)
            throw new InvalidDataException($"Configuration must contain exactly one '{key}' entry.");
        if (!int.TryParse(matches[0].Groups["value"].Value, out var value) || value < min || value > max)
            throw new InvalidDataException($"Configuration value '{key}' is invalid.");
        return value;
    }

    private static string ReplaceUnique(string text, string key, int value)
    {
        var regex = ValueRegex(key);
        if (regex.Matches(text).Count != 1)
            throw new InvalidDataException($"Configuration must contain exactly one '{key}' entry.");
        return regex.Replace(text, m => m.Groups["prefix"].Value + value + m.Groups["suffix"].Value, 1);
    }

    private static Regex ValueRegex(string key) => new(
        $@"(?im)^(?<prefix>\s*""{Regex.Escape(key)}""\s+""?)(?<value>-?\d+)(?<suffix>""?.*)$",
        RegexOptions.CultureInvariant);

    private static Regex BackupNameRegex(string prefix) => new(
        "^" + Regex.Escape(prefix) + @"\d{8}-\d{6}(?:-\d+)?\.bak$",
        RegexOptions.IgnoreCase | RegexOptions.CultureInvariant);

    private static string NewBackupPath(string path)
    {
        var stamp = DateTime.Now.ToString("yyyyMMdd-HHmmss");
        var stem = path + "." + stamp + ".bak";
        if (!File.Exists(stem)) return stem;
        for (var i = 1; ; i++)
        {
            var candidate = path + "." + stamp + $"-{i}.bak";
            if (!File.Exists(candidate)) return candidate;
        }
    }

    private static (DateTime Stamp, int Sequence) BackupOrderKey(string fileName, string prefix)
    {
        var rest = fileName.StartsWith(prefix, StringComparison.OrdinalIgnoreCase)
            ? fileName[prefix.Length..]
            : fileName;
        if (rest.EndsWith(".bak", StringComparison.OrdinalIgnoreCase))
            rest = rest[..^4];
        var sequence = 0;
        var dash = rest.LastIndexOf('-');
        var stampText = rest;
        if (dash >= 0 && rest[(dash + 1)..].All(char.IsDigit) && rest[..dash].Length == "yyyyMMdd-HHmmss".Length)
        {
            if (int.TryParse(rest[(dash + 1)..], out var parsed)) sequence = parsed;
            stampText = rest[..dash];
        }
        if (System.Globalization.CultureInfo.InvariantCulture is var culture &&
            DateTime.TryParseExact(stampText, "yyyyMMdd-HHmmss", culture,
                System.Globalization.DateTimeStyles.None, out var stamp))
            return (stamp, sequence);
        return (DateTime.MinValue, sequence);
    }

    private static void ClearReadOnly(string path)
    {
        try
        {
            if (!File.Exists(path)) return;
            var attributes = File.GetAttributes(path);
            if ((attributes & FileAttributes.ReadOnly) != 0)
                File.SetAttributes(path, attributes & ~FileAttributes.ReadOnly);
        }
        catch (Exception) { }
    }

    private void RetainBackups(string path)
    {
        foreach (var backup in GetBackups(path).Skip(5))
        {
            ClearReadOnly(backup.Path);
            try { File.Delete(backup.Path); }
            catch (Exception) { }
        }
    }

    private static void AtomicWrite(string path, byte[] bytes)
    {
        var directory = Path.GetDirectoryName(path)!;
        var temporary = Path.Combine(directory, $".{Path.GetFileName(path)}.{Guid.NewGuid():N}.tmp");
        File.WriteAllBytes(temporary, bytes);
        try
        {
            if (File.Exists(path))
            {
                ClearReadOnly(path);
                File.Replace(temporary, path, null);
            }
            else File.Move(temporary, path);
        }
        finally
        {
            if (File.Exists(temporary)) File.Delete(temporary);
        }
    }

    private sealed record TextDocument(string Text, Encoding Encoding, bool HasBom)
    {
        public static TextDocument Load(string path)
        {
            var bytes = File.ReadAllBytes(path);
            if (bytes.Length >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE)
                return new TextDocument(Encoding.Unicode.GetString(bytes, 2, bytes.Length - 2), Encoding.Unicode, true);
            if (bytes.Length >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF)
                return new TextDocument(Encoding.BigEndianUnicode.GetString(bytes, 2, bytes.Length - 2), Encoding.BigEndianUnicode, true);
            if (bytes.Length >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF)
                return new TextDocument(Encoding.UTF8.GetString(bytes, 3, bytes.Length - 3), Encoding.UTF8, true);
            if (LooksUtf16(bytes, littleEndian: true))
                return new TextDocument(Encoding.Unicode.GetString(bytes), Encoding.Unicode, false);
            if (LooksUtf16(bytes, littleEndian: false))
                return new TextDocument(Encoding.BigEndianUnicode.GetString(bytes), Encoding.BigEndianUnicode, false);
            return new TextDocument(new UTF8Encoding(false, true).GetString(bytes), new UTF8Encoding(false), false);
        }

        public byte[] Encode(string text)
        {
            var body = Encoding.GetBytes(text);
            if (!HasBom) return body;
            return Encoding.GetPreamble().Concat(body).ToArray();
        }

        private static bool LooksUtf16(byte[] bytes, bool littleEndian)
        {
            if (bytes.Length < 4) return false;
            var zeroes = 0;
            for (var i = littleEndian ? 1 : 0; i < Math.Min(bytes.Length, 200); i += 2)
                if (bytes[i] == 0) zeroes++;
            return zeroes >= Math.Min(bytes.Length, 200) / 8;
        }
    }
}

using Softhe.CS2ResEdit.Core;
using System.Text;

namespace Softhe.CS2ResEdit.Tests;

public sealed class SteamAndPreferencesTests : IDisposable
{
    private readonly string root = Path.Combine(Path.GetTempPath(), "cs2-steam-tests-" + Guid.NewGuid().ToString("N"));

    public SteamAndPreferencesTests() => Directory.CreateDirectory(root);

    [Fact]
    public void ConvertsAccountId() =>
        Assert.Equal(76561198083722595UL, SteamService.ToSteamId64(123456867));

    [Fact]
    public void DiscoversAndOrdersSteamAccounts()
    {
        var config = Path.Combine(root, "userdata", "123456867", "730", "local", "cfg", "cs2_video.txt");
        Directory.CreateDirectory(Path.GetDirectoryName(config)!);
        File.WriteAllText(config, ValidConfig(), Encoding.UTF8);
        Directory.CreateDirectory(Path.Combine(root, "config"));
        File.WriteAllText(Path.Combine(root, "config", "loginusers.vdf"),
            "\"users\"\n{\n\"76561198083722595\"\n{\n\"AccountName\" \"tester\"\n\"PersonaName\" \"Test Person\"\n\"MostRecent\" \"1\"\n}\n}");
        var account = Assert.Single(new SteamService().GetAccounts([root]));
        Assert.Equal("Test Person", account.PersonaName);
        Assert.True(account.MostRecent);
        Assert.True(account.HasConfig);
    }

    [Fact]
    public void ParsesNestedEscapedAndDuplicateLoginUsers()
    {
        var loginUsers = Path.Combine(root, "loginusers.vdf");
        File.WriteAllText(loginUsers, """
            // Steam metadata
            "users"
            {
                "76561198083722595"
                {
                    "AccountName" "tester"
                    "PersonaName" "First"
                    "MostRecent" "0"
                }
                "76561198083722595"
                {
                    "AccountName" "tester"
                    "PersonaName" "Test \"Player\""
                    "MostRecent" "1"
                    "nested" { "ignored" "yes" }
                }
            }
            """
        );

        var user = Assert.Single(new SteamService().ParseLoginUsers(loginUsers));
        Assert.Equal("Test \"Player\"", user.PersonaName);
        Assert.True(user.MostRecent);
    }

    [Fact]
    public void MalformedLoginUsersDoesNotBreakDiscovery()
    {
        var loginUsers = Path.Combine(root, "malformed.vdf");
        File.WriteAllText(loginUsers, "\"users\" { \"123\" {");
        Assert.Empty(new SteamService().ParseLoginUsers(loginUsers));
    }

    [Fact]
    public void ReadsSchemaOneAndCleansRecentFiles()
    {
        var config = Path.Combine(root, "cs2_video.txt");
        File.WriteAllText(config, ValidConfig());
        var settings = Path.Combine(root, "settings.json");
        File.WriteAllText(settings, $$"""{"SchemaVersion":1,"LastAccountId":"42","RecentConfigPaths":["{{config.Replace("\\", "\\\\")}}","missing"]}""");
        var service = new PreferencesService(new VideoConfigService());
        var value = service.Read(settings);
        Assert.Equal("42", value.LastAccountId);
        Assert.Equal(Path.GetFullPath(config), Assert.Single(value.RecentConfigPaths));

        service.Save("43", [config], settings);
        Assert.Equal("43", service.Read(settings).LastAccountId);
    }

    [Fact]
    public void ReportsUnsupportedPreferenceSchema()
    {
        var settings = Path.Combine(root, "settings.json");
        File.WriteAllText(settings, """{"SchemaVersion":2}""");
        var value = new PreferencesService(new VideoConfigService()).Read(settings);
        Assert.NotNull(value.Warning);
        Assert.Empty(value.RecentConfigPaths);
    }

    [Fact]
    public void UsesVersionedDefaultPreferenceDirectory() =>
        Assert.EndsWith(
            Path.Combine("Softhe", "CS2-ResEdit", "v1", "settings.json"),
            PreferencesService.DefaultPath,
            StringComparison.OrdinalIgnoreCase);

    [Fact]
    public void FreshV1PreferencesIgnoreAndPreservePreviousFiles()
    {
        var previousProductPath = Path.Combine(root, "CS2-ResEdit", "settings.json");
        var previousNamePath = Path.Combine(root, "CS2-VideoConfig-Editor", "settings.json");
        var v1Path = Path.Combine(root, "CS2-ResEdit", "v1", "settings.json");
        Directory.CreateDirectory(Path.GetDirectoryName(previousProductPath)!);
        Directory.CreateDirectory(Path.GetDirectoryName(previousNamePath)!);
        const string previousSettings = "{\"SchemaVersion\":1,\"LastAccountId\":\"previous\"}";
        File.WriteAllText(previousProductPath, previousSettings);
        File.WriteAllText(previousNamePath, previousSettings);

        var value = new PreferencesService(new VideoConfigService()).Read(v1Path);

        Assert.Null(value.LastAccountId);
        Assert.False(File.Exists(v1Path));
        Assert.Equal(previousSettings, File.ReadAllText(previousProductPath));
        Assert.Equal(previousSettings, File.ReadAllText(previousNamePath));
    }

    private static string ValidConfig() =>
        "\"setting.defaultres\" \"1920\"\n\"setting.defaultresheight\" \"1080\"\n\"setting.aspectratiomode\" \"1\"\n";

    public void Dispose()
    {
        Directory.Delete(root, true);
        GC.SuppressFinalize(this);
    }
}

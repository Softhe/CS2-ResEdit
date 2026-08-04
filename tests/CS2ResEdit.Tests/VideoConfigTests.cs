using Softhe.CS2ResEdit.Core;
using System.Text;

namespace Softhe.CS2ResEdit.Tests;

public sealed class VideoConfigTests : IDisposable
{
    private readonly string root = Path.Combine(Path.GetTempPath(), "cs2-editor-tests-" + Guid.NewGuid().ToString("N"));
    private readonly VideoConfigService service = new();
    private const string Config = "\"setting.defaultres\"\t\t\"1920\"\r\n\"setting.defaultresheight\"\t\t\"1080\"\r\n\"setting.aspectratiomode\"\t\t\"1\"\r\n\"unrelated\"\t\t\"keep\"\r\n";

    public VideoConfigTests() => Directory.CreateDirectory(root);

    [Fact]
    public void ReadsAllRequiredFields() =>
        Assert.Equal(new VideoConfigState(1920, 1080, 1), service.Parse(Config));

    [Fact]
    public void RejectsMissingAndDuplicateFields()
    {
        Assert.Throws<InvalidDataException>(() => service.Parse(Config.Replace("\"setting.defaultresheight\"\t\t\"1080\"\r\n", "")));
        Assert.Throws<InvalidDataException>(() => service.Parse(Config + "\"setting.defaultres\"\t\"800\"\r\n"));
    }

    [Theory]
    [InlineData("utf8", false)]
    [InlineData("utf8", true)]
    [InlineData("utf16le", false)]
    [InlineData("utf16le", true)]
    [InlineData("utf16be", false)]
    [InlineData("utf16be", true)]
    public void PreservesEncodingBomAndLineEndings(string encodingName, bool bom)
    {
        var encoding = encodingName switch
        {
            "utf16le" => Encoding.Unicode,
            "utf16be" => Encoding.BigEndianUnicode,
            _ => new UTF8Encoding(bom)
        };
        var path = Path.Combine(root, encodingName + "-" + bom + ".txt");
        var text = Config.Replace("\r\n", bom ? "\n" : "\r\n");
        var bytes = (bom ? encoding.GetPreamble() : []).Concat(encoding.GetBytes(text)).ToArray();
        File.WriteAllBytes(path, bytes);

        service.Update(path, ResolutionCatalog.Parse("1280x720"), false);

        var output = File.ReadAllBytes(path);
        Assert.Equal(bom, output.AsSpan().StartsWith(encoding.GetPreamble()) && encoding.GetPreamble().Length > 0);
        var decoded = encoding.GetString(output[(bom ? encoding.GetPreamble().Length : 0)..]);
        Assert.Contains(bom ? "\n" : "\r\n", decoded);
        Assert.Contains("\"unrelated\"\t\t\"keep\"", decoded);
        Assert.Equal(new VideoConfigState(1280, 720, 1), service.Read(path));
    }

    [Fact]
    public void CreatesRestoresAndRetainsRecognizedBackups()
    {
        var path = WriteConfig();
        for (var i = 0; i < 7; i++)
        {
            service.Update(path, ResolutionCatalog.Parse($"{1280 + i}x720"), true);
            Thread.Sleep(5);
        }
        File.WriteAllText(path + ".manual.bak", "do not delete");
        var backups = service.GetBackups(path);
        Assert.Equal(5, backups.Count);
        Assert.True(File.Exists(path + ".manual.bak"));
        var restore = backups.Last();
        var rollback = service.Restore(path, restore.Path);
        Assert.True(File.Exists(rollback));
        Assert.Equal(restore.State, service.Read(path));
    }

    [Fact]
    public void DoesNotWriteWhenStateIsUnchanged()
    {
        var path = WriteConfig();
        var before = File.GetLastWriteTimeUtc(path);
        var result = service.Update(path, ResolutionCatalog.Parse("1920x1080"), true);
        Assert.False(result.Changed);
        Assert.Empty(service.GetBackups(path));
        Assert.Equal(before, File.GetLastWriteTimeUtc(path));
    }

    private string WriteConfig()
    {
        var path = Path.Combine(root, "cs2_video.txt");
        File.WriteAllText(path, Config, new UTF8Encoding(false));
        return path;
    }

    public void Dispose()
    {
        Directory.Delete(root, true);
        GC.SuppressFinalize(this);
    }
}

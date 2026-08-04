using Softhe.CS2ResEdit.Core;

namespace Softhe.CS2ResEdit.Tests;

public sealed class ResolutionTests
{
    [Theory]
    [InlineData("1920x1080", 1920, 1080, 1)]
    [InlineData(" 1280 X 1024 ", 1280, 1024, 0)]
    [InlineData("1440×900", 1440, 900, 2)]
    public void ParsesValidDimensions(string input, int width, int height, int mode)
    {
        var value = ResolutionCatalog.Parse(input);
        Assert.Equal((width, height, mode), (value.Width, value.Height, value.Mode));
    }

    [Theory]
    [InlineData("")]
    [InlineData("1920")]
    [InlineData("1x1")]
    [InlineData("40000x1080")]
    public void RejectsInvalidDimensions(string input) =>
        Assert.Throws<ArgumentException>(() => ResolutionCatalog.Parse(input));

    [Fact]
    public void PresetCatalogIsExpandedAndHasUniqueDimensions()
    {
        Assert.True(ResolutionCatalog.Presets.Count >= 40);
        Assert.Equal(ResolutionCatalog.Presets.Count,
            ResolutionCatalog.Presets.Select(x => (x.Width, x.Height)).Distinct().Count());
    }

    [Theory]
    [InlineData(800, 600, 0)]
    [InlineData(1024, 768, 0)]
    [InlineData(1920, 1080, 1)]
    [InlineData(2560, 1440, 1)]
    [InlineData(1280, 800, 2)]
    [InlineData(1920, 1200, 2)]
    public void ContainsCommonResolutionPresets(int width, int height, int mode) =>
        Assert.Contains(ResolutionCatalog.Presets,
            preset => preset.Width == width && preset.Height == height && preset.Mode == mode);

    [Theory]
    [InlineData(0, 1280, 960)]
    [InlineData(1, 1920, 1080)]
    [InlineData(2, 1680, 1050)]
    public void ProvidesRecommendedPresetForEachAspectMode(int mode, int width, int height)
    {
        var preset = ResolutionCatalog.RecommendedPreset(mode);
        Assert.Equal((width, height, mode), (preset.Width, preset.Height, preset.Mode));
    }
}

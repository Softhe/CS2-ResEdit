using System.Globalization;
using System.Text.RegularExpressions;

namespace Softhe.CS2ResEdit.Core;

public static partial class ResolutionCatalog
{
    public static IReadOnlyList<Resolution> Presets { get; } =
    [
        R(640,480,"4:3",0), R(720,540,"4:3",0), R(800,600,"4:3",0),
        R(960,720,"4:3",0), R(1024,768,"4:3",0), R(1152,864,"4:3",0),
        R(1280,960,"4:3",0), R(1280,1024,"5:4",0), R(1400,1050,"4:3",0),
        R(1440,1080,"4:3",0), R(1600,1200,"4:3",0), R(1920,1440,"4:3",0),
        R(2048,1536,"4:3",0), R(2560,1920,"4:3",0), R(2880,2160,"4:3",0),

        R(854,480,"16:9",1), R(960,540,"16:9",1), R(1024,576,"16:9",1),
        R(1152,648,"16:9",1), R(1280,720,"16:9",1), R(1366,768,"16:9",1),
        R(1600,900,"16:9",1), R(1920,1080,"16:9",1), R(2048,1152,"16:9",1),
        R(2560,1440,"16:9",1), R(2880,1620,"16:9",1), R(3200,1800,"16:9",1),
        R(3840,2160,"16:9",1),

        R(640,400,"16:10",2), R(800,500,"16:10",2), R(960,600,"16:10",2),
        R(1024,640,"16:10",2), R(1152,720,"16:10",2), R(1280,800,"16:10",2),
        R(1440,900,"16:10",2), R(1680,1050,"16:10",2), R(1728,1080,"16:10",2),
        R(1920,1200,"16:10",2), R(2304,1440,"16:10",2), R(2560,1600,"16:10",2),
        R(2880,1800,"16:10",2), R(3456,2160,"16:10",2), R(3840,2400,"16:10",2)
    ];

    public const int MinWidth = 320;
    public const int MaxDimension = 32768;
    public const int MinHeight = 200;
    internal const int MaxInputLength = 64;

    /// <summary>Tolerance for recognizing the 5:4 ratio inside aspect mode 0.</summary>
    private const double FiveFourTolerance = 0.03;
    /// <summary>Relative deviation that counts as stretched for a given aspect mode.</summary>
    private const double StretchTolerance = 0.06;

    public static Resolution RecommendedPreset(int mode) => mode switch
    {
        0 => Presets.FirstOrDefault(x => x.Width == 1280 && x.Height == 960) ?? new Resolution(1280, 960, "4:3", 0),
        1 => Presets.FirstOrDefault(x => x.Width == 1920 && x.Height == 1080) ?? new Resolution(1920, 1080, "16:9", 1),
        2 => Presets.FirstOrDefault(x => x.Width == 1680 && x.Height == 1050) ?? new Resolution(1680, 1050, "16:10", 2),
        _ => throw new ArgumentOutOfRangeException(nameof(mode))
    };

    public static Resolution Parse(string? value, int? aspectMode = null)
    {
        if (string.IsNullOrWhiteSpace(value) || value.Length > MaxInputLength)
            throw new ArgumentException("Enter a resolution as WIDTHxHEIGHT (320–32768 by 200–32768).");
        var match = ResolutionRegex().Match(value.Trim());
        if (!match.Success ||
            !int.TryParse(match.Groups[1].Value, CultureInfo.InvariantCulture, out var width) ||
            !int.TryParse(match.Groups[2].Value, CultureInfo.InvariantCulture, out var height) ||
            width < MinWidth || width > MaxDimension || height < MinHeight || height > MaxDimension)
            throw new ArgumentException("Enter a resolution as WIDTHxHEIGHT (320–32768 by 200–32768).");

        var preset = Presets.FirstOrDefault(x => x.Width == width && x.Height == height);
        var mode = aspectMode ?? preset?.Mode ?? AutomaticAspectMode(width, height);
        return new Resolution(width, height, ModeName(mode, width, height), mode);
    }

    /// <summary>
    /// Maps dimensions to the nearest of the three aspect modes CS2 supports.
    /// Ultrawide widths (e.g. 21:9) have no dedicated mode and map to the
    /// closest family; <see cref="IsStretched"/> warns when the deviation
    /// would appear stretched.
    /// </summary>
    public static int AutomaticAspectMode(int width, int height)
    {
        var ratio = (double)width / height;
        var choices = new[] { (Mode: 0, Ratio: 4d / 3d), (Mode: 1, Ratio: 16d / 9d), (Mode: 2, Ratio: 16d / 10d) };
        return choices.MinBy(x => Math.Abs(x.Ratio - ratio)).Mode;
    }

    public static bool IsStretched(int width, int height, int mode)
    {
        if (width <= 0 || height <= 0) return false;
        var actual = (double)width / height;
        if (mode == 0 && Math.Abs(actual - 1.25) < FiveFourTolerance) return false;
        var expected = mode switch { 0 => 4d / 3d, 1 => 16d / 9d, 2 => 16d / 10d, _ => actual };
        return Math.Abs(actual - expected) / expected > StretchTolerance;
    }

    public static string ModeName(int mode, int width = 4, int height = 3) => mode switch
    {
        0 when Math.Abs((double)width / height - 1.25) < FiveFourTolerance => "5:4",
        0 => "4:3",
        1 => "16:9",
        2 => "16:10",
        _ => throw new ArgumentOutOfRangeException(nameof(mode))
    };

    private static Resolution R(int w, int h, string ratio, int mode) => new(w, h, ratio, mode);

    [GeneratedRegex(@"^\s*(\d+)\s*[xX×]\s*(\d+)\s*$", RegexOptions.CultureInvariant)]
    private static partial Regex ResolutionRegex();
}

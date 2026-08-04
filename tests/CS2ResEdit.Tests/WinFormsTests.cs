using Softhe.CS2ResEdit.Editor;
using Softhe.CS2ResEdit.Core;
using System.Reflection;

namespace Softhe.CS2ResEdit.Tests;

public sealed class WinFormsTests
{
    [Fact]
    public void ExposesKeyboardAndAccessibilityContracts() => RunSta(() =>
    {
        using var form = new MainForm(Path.Combine(Path.GetTempPath(), Guid.NewGuid() + ".json"), null,
            new FakeDisplayProvider([]));
        Assert.NotNull(Find(form, "Steam account"));
        Assert.NotNull(Find(form, "Refresh Steam accounts"));
        Assert.NotNull(Find(form, "Browse for cs2_video.txt"));
        Assert.NotNull(Find(form, "Resolution preset"));
        Assert.NotNull(Find(form, "Custom width"));
        Assert.NotNull(Find(form, "Custom height"));
        Assert.NotNull(Find(form, "Aspect ratio"));
        Assert.NotNull(Find(form, "Resolution aspect preview"));
        Assert.Contains(Descendants(form).OfType<Label>(), label => label.Text == "Preview");
        Assert.DoesNotContain(Descendants(form).OfType<Label>(), label => label.Text == "Review");
        Assert.NotNull(Find(form, "Apply pending changes"));
        Assert.Same(Find(form, "Apply pending changes"), form.AcceptButton);
        Assert.True(form.AutoScaleMode == AutoScaleMode.Dpi);
        Assert.True(form.KeyPreview);
    });

    [Fact]
    public void ResetAndApplyReflectPendingState() => RunSta(() =>
    {
        var root = Path.Combine(Path.GetTempPath(), "cs2-ui-test-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(root);
        try
        {
            var config = Path.Combine(root, "cs2_video.txt");
            File.WriteAllText(config,
                "\"setting.defaultres\" \"1920\"\n\"setting.defaultresheight\" \"1080\"\n\"setting.aspectratiomode\" \"1\"\n");
            using var form = new MainForm(Path.Combine(root, "settings.json"));
            Invoke(form, "InitializeData");
            Invoke(form, "LoadPath", config);
            var apply = Assert.IsAssignableFrom<Button>(Find(form, "Apply pending changes"));
            var reset = Assert.IsAssignableFrom<Button>(Find(form, "Reset pending changes"));
            Assert.False(apply.Enabled);
            Assert.False(reset.Enabled);
            Assert.IsType<TextBox>(Find(form, "Custom width")).Text = "1280";
            Assert.True(apply.Enabled);
            Assert.True(reset.Enabled);
            Assert.Equal(ThemePalette.Create(SystemInformation.HighContrast).Accent, apply.BackColor);
        }
        finally { Directory.Delete(root, true); }
    });

    [Fact]
    public void ReflowsAtNarrowAndWideWidths() => RunSta(() =>
    {
        using var form = new MainForm(Path.Combine(Path.GetTempPath(), Guid.NewGuid() + ".json"));
        var body = Assert.IsAssignableFrom<TableLayoutPanel>(Find(form, "Editor content"));
        form.ClientSize = new Size(700, 650);
        Invoke(form, "UpdateResponsiveLayout");
        Assert.Equal(1, body.ColumnCount);
        form.ClientSize = new Size(1200, 650);
        Invoke(form, "UpdateResponsiveLayout");
        Assert.Equal(2, body.ColumnCount);
    });

    [Fact]
    public void AspectSelectionFiltersPresetCatalog() => RunSta(() =>
    {
        using var form = new MainForm(Path.Combine(Path.GetTempPath(), Guid.NewGuid() + ".json"), null,
            new FakeDisplayProvider([]));
        Invoke(form, "InitializeData");
        var aspect = Assert.IsAssignableFrom<ComboBox>(Find(form, "Aspect ratio"));
        var presets = Assert.IsAssignableFrom<ComboBox>(Find(form, "Resolution preset"));

        aspect.SelectedIndex = 1;
        Assert.Equal(ResolutionCatalog.Presets.Count(x => x.Mode == 1) + 1, presets.Items.Count);
        Assert.All(presets.Items.Cast<object>().Skip(1), item => Assert.Contains("(16:9)", item.ToString()));
        Assert.StartsWith("1920x1080", presets.SelectedItem!.ToString());
        Assert.Equal("1920", Assert.IsType<TextBox>(Find(form, "Custom width")).Text);
        Assert.Equal("1080", Assert.IsType<TextBox>(Find(form, "Custom height")).Text);

        aspect.SelectedIndex = 2;
        Assert.Equal(ResolutionCatalog.Presets.Count(x => x.Mode == 2) + 1, presets.Items.Count);
        Assert.All(presets.Items.Cast<object>().Skip(1), item => Assert.Contains("(16:10)", item.ToString()));
        Assert.StartsWith("1680x1050", presets.SelectedItem!.ToString());

        aspect.SelectedIndex = 0;
        Assert.Equal(ResolutionCatalog.Presets.Count(x => x.Mode == 0) + 1, presets.Items.Count);
        Assert.All(presets.Items.Cast<object>().Skip(1), item =>
            Assert.True(item.ToString()!.Contains("(4:3)") || item.ToString()!.Contains("(5:4)")));
        Assert.StartsWith("1280x960", presets.SelectedItem!.ToString());
    });

    [Fact]
    public void SupportedDisplayModesAreListedBeforeCatalogOnlyModes() => RunSta(() =>
    {
        var provider = new FakeDisplayProvider([
            new DisplayInfo("DISPLAY1", "Fixture Display", true, 1920, 1080,
                [new DisplayResolution(1280, 720), new DisplayResolution(1360, 768), new DisplayResolution(1920, 1080)])
        ]);
        using var form = new MainForm(Path.Combine(Path.GetTempPath(), Guid.NewGuid() + ".json"), null, provider);
        Invoke(form, "InitializeData");
        var display = Assert.IsAssignableFrom<ComboBox>(Find(form, "Target display"));
        var aspect = Assert.IsAssignableFrom<ComboBox>(Find(form, "Aspect ratio"));
        var presets = Assert.IsAssignableFrom<ComboBox>(Find(form, "Resolution preset"));

        Assert.Contains("Fixture Display", display.Text);
        var choicesProperty = display.GetType().GetProperty("HasMultipleChoices",
            BindingFlags.Instance | BindingFlags.NonPublic)!;
        Assert.False(Assert.IsType<bool>(choicesProperty.GetValue(display)));
        Assert.True(Assert.IsType<bool>(choicesProperty.GetValue(aspect)));
        aspect.SelectedIndex = 1;
        Assert.Contains("available", presets.Items[1]!.ToString());
        Assert.Contains("available", presets.Items[2]!.ToString());
        Assert.Contains(presets.Items.Cast<object>(), item => item.ToString()!.StartsWith("1360x768") && item.ToString()!.Contains("available"));
        Assert.Contains(presets.Items.Cast<object>(), item => item.ToString()!.Contains("not reported"));
        Assert.NotNull(Find(form, "Display mode availability"));
        Assert.NotNull(Find(form, "Resolution validation message"));
        Assert.NotNull(Find(form, "Open privacy-safe diagnostics"));
    });

    [Fact]
    public void TopLevelWindowsUseDoubleBufferingWithoutWindowWideCompositing() => RunSta(() =>
    {
        using var form = new MainForm(Path.Combine(Path.GetTempPath(), Guid.NewGuid() + ".json"));
        Assert.IsAssignableFrom<BufferedForm>(form);

        using var probe = new BufferedFormProbe();
        Assert.True(probe.UsesOptimizedDoubleBuffer);
        Assert.True(probe.UsesAllPaintingInWmPaint);
        Assert.Equal(0, probe.ExtendedStyle & 0x02000000);
    });

    [Fact]
    public void BalancedThemeMeetsContrastTargets()
    {
        var theme = ThemePalette.Create(false);
        Assert.Equal(Color.FromArgb(18, 20, 22), theme.Window);
        Assert.Equal(Color.FromArgb(28, 32, 35), theme.Card);
        Assert.Equal(Color.FromArgb(21, 25, 28), theme.Input);
        Assert.Equal(Color.FromArgb(248, 157, 28), theme.Accent);
        Assert.True(Contrast(theme.Text, theme.Card) >= 4.5);
        Assert.True(Contrast(theme.Muted, theme.Card) >= 4.5);
        Assert.True(Contrast(theme.Border, theme.Card) >= 1.5);
        Assert.True(Contrast(theme.BorderStrong, theme.Input) >= 3);
        Assert.True(Contrast(theme.Accent, theme.Input) >= 3);
        Assert.True(Contrast(theme.AccentText, theme.Accent) >= 4.5);

        var highContrast = ThemePalette.Create(true);
        Assert.Equal(SystemColors.Window, highContrast.Window);
        Assert.Equal(SystemColors.WindowText, highContrast.Text);
        Assert.Equal(SystemColors.Highlight, highContrast.Accent);
        Assert.Equal(SystemColors.HighlightText, highContrast.AccentText);
    }

    [Fact]
    public void ComfortableDensityKeepsSmallTextReadable() => RunSta(() =>
    {
        using var form = new MainForm(Path.Combine(Path.GetTempPath(), Guid.NewGuid() + ".json"));
        Assert.True(form.Font.Size >= 10.5F);
        var labels = Descendants(form).OfType<Label>().ToList();
        Assert.True(labels.Single(label => label.Text == "STEAM ACCOUNT").Font.Size >= 9F);
        Assert.True(labels.Single(label => label.Text == "LIVE ASPECT PREVIEW").Font.Size >= 9F);
        Assert.DoesNotContain(labels, label => label.Text == "×");
        var backupsButton = Assert.IsAssignableFrom<Button>(Find(form, "Browse and restore configuration backups"));
        Assert.Equal(typeof(Button), backupsButton.GetType());
        Assert.Equal(new Size(132, 38), backupsButton.Size);
        Assert.False(backupsButton.AutoSize);
        Assert.Equal(ThemePalette.Create(SystemInformation.HighContrast).Card, backupsButton.BackColor);
        Assert.Equal(ThemePalette.Create(SystemInformation.HighContrast).BorderStrong,
            backupsButton.FlatAppearance.BorderColor);
        var applyButton = Assert.IsAssignableFrom<Button>(Find(form, "Apply pending changes"));
        Assert.True(applyButton.Font.Size >= 9.5F);
        Assert.False(applyButton.Enabled);
        Assert.Equal(ThemePalette.Create(SystemInformation.HighContrast).Muted, applyButton.ForeColor);
        Assert.Equal(ThemePalette.Create(SystemInformation.HighContrast).BorderStrong,
            applyButton.FlatAppearance.BorderColor);
        Assert.True(Assert.IsAssignableFrom<ComboBox>(Find(form, "Resolution preset")).ItemHeight >= 30);
    });

    [Fact]
    public void CustomDimensionsOnlyParticipateInLayoutForCustomPreset() => RunSta(() =>
    {
        var root = Path.Combine(Path.GetTempPath(), "cs2-custom-ui-test-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(root);
        try
        {
            var config = Path.Combine(root, "cs2_video.txt");
            File.WriteAllText(config,
                "\"setting.defaultres\" \"1920\"\n\"setting.defaultresheight\" \"1080\"\n\"setting.aspectratiomode\" \"1\"\n");
            using var form = new MainForm(Path.Combine(root, "settings.json"), null, new FakeDisplayProvider([]));
            Invoke(form, "LoadPath", config);

            var settings = Assert.IsAssignableFrom<TableLayoutPanel>(Find(form, "Configuration settings"));
            var presets = Assert.IsAssignableFrom<ComboBox>(Find(form, "Resolution preset"));
            var width = Assert.IsAssignableFrom<TextBox>(Find(form, "Custom width"));
            var height = Assert.IsAssignableFrom<TextBox>(Find(form, "Custom height"));

            Assert.True(presets.SelectedIndex > 0);
            Assert.Equal(0, settings.RowStyles[8].Height);
            Assert.False(width.TabStop);
            Assert.False(height.TabStop);

            presets.SelectedIndex = 0;
            Assert.Equal(86, settings.RowStyles[8].Height);
            Assert.True(width.TabStop);
            Assert.True(height.TabStop);
            Assert.Equal("1920", width.Text);
            Assert.Equal("1080", height.Text);

            width.Text = "1111";
            height.Text = "777";
            presets.SelectedIndex = 1;
            Assert.Equal(0, settings.RowStyles[8].Height);
            presets.SelectedIndex = 0;
            Assert.Equal("1920", width.Text);
            Assert.Equal("1080", height.Text);
        }
        finally { Directory.Delete(root, true); }
    });

    [Fact]
    public void RoundedControlsUseAntialiasedPaintingInsteadOfHardClippingRegions() => RunSta(() =>
    {
        using var form = new MainForm(Path.Combine(Path.GetTempPath(), Guid.NewGuid() + ".json"));
        form.CreateControl();

        var roundedControls = Descendants(form)
            .Where(control => control.GetType().Name == "RoundedPanel")
            .ToList();

        Assert.NotEmpty(roundedControls);
        Assert.All(roundedControls, control => Assert.Null(control.Region));
    });

    [Fact]
    public void PreviewUsesOneStaticScaleAcrossStandardAspectFamilies()
    {
        var method = typeof(MainForm).GetMethod("CalculatePreviewRectangle",
            BindingFlags.Static | BindingFlags.NonPublic)!;
        var viewportMethod = typeof(MainForm).GetMethod("CalculatePreviewViewport",
            BindingFlags.Static | BindingFlags.NonPublic)!;
        var availableBounds = new Rectangle(0, 0, 360, 260);
        var bounds = Assert.IsType<RectangleF>(viewportMethod.Invoke(null, [availableBounds]));
        RectangleF Calculate(int width, int height) =>
            Assert.IsType<RectangleF>(method.Invoke(null, [bounds, width, height]));

        var fiveByFour = Calculate(1280, 1024);
        var fourByThree = Calculate(1280, 960);
        var sixteenByTen = Calculate(1680, 1050);
        var sixteenByNine = Calculate(1920, 1080);

        Assert.Equal(fiveByFour.Height, fourByThree.Height, 3);
        Assert.Equal(fourByThree.Height, sixteenByTen.Height, 3);
        Assert.Equal(sixteenByTen.Height, sixteenByNine.Height, 3);
        Assert.True(fiveByFour.Width < fourByThree.Width);
        Assert.True(fourByThree.Width < sixteenByTen.Width);
        Assert.True(sixteenByTen.Width < sixteenByNine.Width);
        Assert.Equal(bounds, sixteenByNine);
        Assert.Equal(bounds.Width / 2F, fiveByFour.Left + fiveByFour.Width / 2F, 3);
        Assert.Equal(bounds.Width / 2F, sixteenByNine.Left + sixteenByNine.Width / 2F, 3);
    }

    [Fact]
    public void DarkDropDownsDoNotExposeTheNativeWhiteSurface() => RunSta(() =>
    {
        using var form = new MainForm(Path.Combine(Path.GetTempPath(), Guid.NewGuid() + ".json"), null,
            new FakeDisplayProvider([]));
        form.CreateControl();

        var dropDowns = Descendants(form).OfType<ComboBox>().ToList();
        Assert.NotEmpty(dropDowns);
        foreach (var dropDown in dropDowns)
        {
            dropDown.CreateControl();
            using var bitmap = new Bitmap(Math.Max(1, dropDown.Width), Math.Max(1, dropDown.Height));
            dropDown.DrawToBitmap(bitmap, new Rectangle(Point.Empty, bitmap.Size));
            if (dropDown.Items.Count > 1 && bitmap.Width > 60 && bitmap.Height > 10)
            {
                var input = ThemePalette.Create(SystemInformation.HighContrast).Input;
                Assert.Equal(input, bitmap.GetPixel(5, 5));
                Assert.Equal(input, bitmap.GetPixel(bitmap.Width - 25, 5));
            }
            for (var y = 0; y < bitmap.Height; y++)
                for (var x = 0; x < bitmap.Width; x++)
                {
                    var pixel = bitmap.GetPixel(x, y);
                    Assert.False(pixel.R > 250 && pixel.G > 250 && pixel.B > 250,
                        $"Native white paint detected in {dropDown.AccessibleName} at {x},{y}.");
                }
        }
    });

    private static Control? Find(Control root, string accessibleName)
    {
        if (root.AccessibleName == accessibleName) return root;
        foreach (Control child in root.Controls)
        {
            var found = Find(child, accessibleName);
            if (found is not null) return found;
        }
        return null;
    }

    private static IEnumerable<Control> Descendants(Control root)
    {
        foreach (Control child in root.Controls)
        {
            yield return child;
            foreach (var descendant in Descendants(child)) yield return descendant;
        }
    }

    private static void Invoke(MainForm form, string name, params object[] args) =>
        form.GetType().GetMethod(name, BindingFlags.Instance | BindingFlags.NonPublic)!.Invoke(form, args);

    private static double Contrast(Color first, Color second)
    {
        static double Luminance(Color color)
        {
            static double Linear(byte value)
            {
                var channel = value / 255D;
                return channel <= 0.04045 ? channel / 12.92 : Math.Pow((channel + 0.055) / 1.055, 2.4);
            }

            return 0.2126 * Linear(color.R) + 0.7152 * Linear(color.G) + 0.0722 * Linear(color.B);
        }

        var firstLuminance = Luminance(first);
        var secondLuminance = Luminance(second);
        return (Math.Max(firstLuminance, secondLuminance) + 0.05) /
            (Math.Min(firstLuminance, secondLuminance) + 0.05);
    }

    private static void RunSta(Action action)
    {
        Exception? error = null;
        var thread = new Thread(() =>
        {
            try { action(); }
            catch (Exception ex) { error = ex; }
        });
        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
        thread.Join();
        if (error is not null) throw error;
    }

    private sealed class BufferedFormProbe : BufferedForm
    {
        public int ExtendedStyle => CreateParams.ExStyle;
        public bool UsesOptimizedDoubleBuffer => GetStyle(ControlStyles.OptimizedDoubleBuffer);
        public bool UsesAllPaintingInWmPaint => GetStyle(ControlStyles.AllPaintingInWmPaint);
    }

    private sealed class FakeDisplayProvider(IReadOnlyList<DisplayInfo> displays) : IDisplayModeProvider
    {
        public IReadOnlyList<DisplayInfo> GetDisplays() => displays;
    }
}

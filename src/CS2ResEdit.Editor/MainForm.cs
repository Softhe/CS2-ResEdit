using Softhe.CS2ResEdit.Core;

namespace Softhe.CS2ResEdit.Editor;

public sealed class MainForm : BufferedForm
{
    private readonly VideoConfigService configs = new();
    private readonly SteamService steam = new();
    private readonly PreferencesService preferenceService;
    private readonly IDisplayModeProvider displayProvider;
    private readonly DiagnosticService diagnosticService;
    private readonly string? settingsPath;
    private readonly string? steamRoot;
    private Preferences preferences = new();
    private readonly ComboBox accounts = new DarkComboBox();
    private readonly ComboBox presets = new DarkComboBox();
    private readonly TextBox customWidth = new();
    private readonly TextBox customHeight = new();
    private readonly ComboBox aspect = new DarkComboBox();
    private readonly ComboBox displays = new DarkComboBox();
    private readonly Label availability = new();
    private readonly Label validation = new();
    private readonly Label current = new();
    private readonly Label pending = new();
    private readonly Label filePath = new();
    private readonly Label status = new();
    private readonly CheckBox createBackup = new();
    private readonly Button apply = new StableFlatButton();
    private readonly Button reset = new StableFlatButton();
    private readonly Panel preview = new();
    private readonly TableLayoutPanel body = new BufferedTableLayoutPanel();
    private readonly Panel contentHost = new();
    private TableLayoutPanel? settingsGrid;
    private TableLayoutPanel? customDimensions;
    private IReadOnlyList<Resolution> displayedPresets = [];
    private HashSet<(int Width, int Height)> supportedModes = [];
    private IReadOnlyList<DisplayInfo> detectedDisplays = [];
    private int discoveredAccountCount;
    private int discoveredRootCount;
    private string? selectedPath;
    private VideoConfigState? original;
    private bool loading;

    public MainForm(string? settingsPath = null, string? steamRoot = null, IDisplayModeProvider? displayProvider = null)
    {
        this.settingsPath = settingsPath;
        this.steamRoot = steamRoot;
        preferenceService = new PreferencesService(configs);
        this.displayProvider = displayProvider ?? new WindowsDisplayModeProvider();
        diagnosticService = new DiagnosticService(configs);
        Text = "CS2 ResEdit";
        MinimumSize = new Size(960, 760);
        Size = new Size(1260, 900);
        StartPosition = FormStartPosition.CenterScreen;
        AutoScaleMode = AutoScaleMode.Dpi;
        Font = new Font("Segoe UI", 10.5F);
        KeyPreview = true;
        BackColor = Palette.Window;
        ForeColor = Palette.Text;
        TryLoadIcon();
        BuildUi();
        Load += (_, _) => InitializeData();
        Shown += (_, _) => ApplyDarkTitleBar();
        Resize += (_, _) => UpdateResponsiveLayout();
        KeyDown += (_, e) => { if (e.KeyCode == Keys.F5) { RefreshAccounts(); e.Handled = true; } };
    }

    private void BuildUi()
    {
        var root = new BufferedTableLayoutPanel
        {
            Dock = DockStyle.Fill,
            Padding = new Padding(32, 24, 32, 20),
            ColumnCount = 1,
            RowCount = 3,
            BackColor = BackColor
        };
        root.RowStyles.Add(new RowStyle(SizeType.Absolute, 82));
        root.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
        root.RowStyles.Add(new RowStyle(SizeType.Absolute, 62));
        Controls.Add(root);

        var header = new BufferedTableLayoutPanel
        {
            Dock = DockStyle.Fill,
            ColumnCount = 2,
            RowCount = 1,
            BackColor = Palette.Window
        };
        header.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        header.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        var brand = new BufferedFlowLayoutPanel
        {
            Dock = DockStyle.Fill,
            FlowDirection = FlowDirection.TopDown,
            WrapContents = false,
            BackColor = Palette.Window,
            Margin = Padding.Empty
        };
        brand.Controls.Add(new Label
        {
            Text = "CS2 ResEdit",
            Font = new Font(Font.FontFamily, 23, FontStyle.Bold),
            AutoSize = true,
            ForeColor = Palette.Text,
            AccessibleName = "Application title"
        });
        brand.Controls.Add(new Label
        {
            Text = "Tune your display settings safely",
            AutoSize = true,
            Font = new Font(Font.FontFamily, 10F),
            ForeColor = Palette.Muted,
            Margin = new Padding(2, 2, 0, 0)
        });
        header.Controls.Add(brand, 0, 0);
        header.Controls.Add(new Label
        {
            Text = "CS2  •  DISPLAY",
            AutoSize = true,
            ForeColor = Palette.Accent,
            Font = new Font(Font.FontFamily, 9, FontStyle.Bold),
            Padding = new Padding(12, 8, 12, 8),
            BackColor = Palette.AccentSoft,
            Margin = new Padding(0, 8, 0, 0)
        }, 1, 0);
        root.Controls.Add(header, 0, 0);

        contentHost.Dock = DockStyle.Fill;
        contentHost.BackColor = Palette.Window;
        contentHost.Controls.Add(body);
        root.Controls.Add(contentHost, 0, 1);

        body.ColumnCount = 2;
        body.RowCount = 1;
        body.AccessibleName = "Editor content";
        body.BackColor = Palette.Window;
        body.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 61));
        body.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 39));
        var settings = Card();
        var summary = Card();
        body.Controls.Add(settings, 0, 0);
        body.Controls.Add(summary, 1, 0);

        settingsGrid = new BufferedTableLayoutPanel
        {
            Dock = DockStyle.Fill,
            ColumnCount = 1,
            RowCount = 11,
            BackColor = Palette.Card,
            Margin = Padding.Empty,
            AccessibleName = "Configuration settings"
        };
        settingsGrid.RowStyles.Add(new RowStyle(SizeType.Absolute, 54));
        settingsGrid.RowStyles.Add(new RowStyle(SizeType.Absolute, 22));
        settingsGrid.RowStyles.Add(new RowStyle(SizeType.Absolute, 44));
        settingsGrid.RowStyles.Add(new RowStyle(SizeType.Absolute, 42));
        settingsGrid.RowStyles.Add(new RowStyle(SizeType.Absolute, 62));
        settingsGrid.RowStyles.Add(new RowStyle(SizeType.Absolute, 1));
        settingsGrid.RowStyles.Add(new RowStyle(SizeType.Absolute, 72));
        settingsGrid.RowStyles.Add(new RowStyle(SizeType.Absolute, 86));
        settingsGrid.RowStyles.Add(new RowStyle(SizeType.Absolute, 86));
        settingsGrid.RowStyles.Add(new RowStyle(SizeType.Absolute, 34));
        settingsGrid.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
        settings.Controls.Add(settingsGrid);
        settingsGrid.Controls.Add(SectionTitle("Configuration", "Choose an account or browse to a config file."), 0, 0);
        settingsGrid.Controls.Add(FieldLabel("STEAM ACCOUNT"), 0, 1);

        var accountRow = new BufferedTableLayoutPanel
        {
            Dock = DockStyle.Fill,
            ColumnCount = 3,
            RowCount = 1,
            BackColor = Palette.Card,
            Margin = Padding.Empty
        };
        accountRow.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        accountRow.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        accountRow.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        ConfigureCombo(accounts, "Steam account");
        accounts.Dock = DockStyle.Fill;
        accounts.SelectedIndexChanged += (_, _) => AccountChanged();
        accountRow.Controls.Add(accounts, 0, 0);
        accountRow.Controls.Add(Button("Refresh", (_, _) => RefreshAccounts(), "Refresh Steam accounts"), 1, 0);
        accountRow.Controls.Add(Button("Browse", (_, _) => Browse(), "Browse for cs2_video.txt"), 2, 0);
        settingsGrid.Controls.Add(accountRow, 0, 2);

        filePath.AutoEllipsis = true;
        filePath.Dock = DockStyle.Fill;
        filePath.ForeColor = Palette.Muted;
        filePath.BackColor = Palette.Input;
        filePath.Padding = new Padding(12, 11, 12, 0);
        filePath.AccessibleName = "Selected configuration path";
        settingsGrid.Controls.Add(filePath, 0, 3);

        settingsGrid.Controls.Add(SectionTitle("Display settings", "Aspect mode filters the available preset list."), 0, 4);
        settingsGrid.Controls.Add(new Panel { Dock = DockStyle.Fill, BackColor = Palette.Border }, 0, 5);

        ConfigureCombo(displays, "Target display");
        displays.Dock = DockStyle.Fill;
        displays.SelectedIndexChanged += (_, _) => DisplayChanged();
        settingsGrid.Controls.Add(Labeled("TARGET DISPLAY", displays), 0, 6);

        var selectionGrid = new BufferedTableLayoutPanel
        {
            Dock = DockStyle.Fill,
            ColumnCount = 2,
            RowCount = 1,
            BackColor = Palette.Card,
            Margin = Padding.Empty
        };
        selectionGrid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 60));
        selectionGrid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 40));
        ConfigureCombo(presets, "Resolution preset");
        presets.Dock = DockStyle.Fill;
        presets.SelectedIndexChanged += (_, _) => PresetChanged();
        ConfigureCombo(aspect, "Aspect ratio");
        aspect.Items.AddRange(["4:3 / 5:4", "16:9", "16:10"]);
        aspect.Dock = DockStyle.Fill;
        aspect.SelectedIndexChanged += (_, _) => AspectChanged();
        selectionGrid.Controls.Add(Labeled("RESOLUTION PRESET", presets), 0, 0);
        selectionGrid.Controls.Add(Labeled("ASPECT RATIO", aspect), 1, 0);
        settingsGrid.Controls.Add(selectionGrid, 0, 7);

        ConfigureText(customWidth, "Custom width");
        ConfigureText(customHeight, "Custom height");
        customWidth.TextChanged += (_, _) => PendingChanged();
        customHeight.TextChanged += (_, _) => PendingChanged();
        customDimensions = new BufferedTableLayoutPanel
        {
            Dock = DockStyle.Fill,
            ColumnCount = 2,
            RowCount = 2,
            BackColor = Palette.Card,
            Margin = Padding.Empty,
            AccessibleName = "Custom resolution dimensions"
        };
        customDimensions.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50));
        customDimensions.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50));
        customDimensions.RowStyles.Add(new RowStyle(SizeType.Absolute, 24));
        customDimensions.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
        customDimensions.Controls.Add(FieldLabel("CUSTOM WIDTH"), 0, 0);
        customDimensions.Controls.Add(FieldLabel("CUSTOM HEIGHT"), 1, 0);
        customWidth.Margin = new Padding(0, 0, 19, 0);
        customHeight.Margin = new Padding(19, 0, 0, 0);
        customDimensions.Controls.Add(customWidth, 0, 1);
        customDimensions.Controls.Add(customHeight, 1, 1);
        settingsGrid.Controls.Add(customDimensions, 0, 8);

        var feedback = new BufferedTableLayoutPanel
        {
            Dock = DockStyle.Fill,
            ColumnCount = 2,
            RowCount = 1,
            BackColor = Palette.Card,
            Margin = Padding.Empty
        };
        feedback.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50));
        feedback.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50));
        availability.Dock = DockStyle.Fill; availability.ForeColor = Palette.Muted;
        availability.AccessibleName = "Display mode availability";
        validation.Dock = DockStyle.Fill; validation.ForeColor = Palette.Warning;
        validation.TextAlign = ContentAlignment.TopRight; validation.AutoEllipsis = true;
        validation.AccessibleName = "Resolution validation message";
        feedback.Controls.Add(availability, 0, 0); feedback.Controls.Add(validation, 1, 0);
        settingsGrid.Controls.Add(feedback, 0, 9);

        createBackup.Text = "Create a timestamped backup before applying";
        createBackup.Checked = true;
        createBackup.AutoSize = true;
        createBackup.AccessibleName = "Create a timestamped backup";
        createBackup.ForeColor = Palette.Text;
        createBackup.FlatStyle = FlatStyle.Flat;
        createBackup.Margin = new Padding(4, 18, 0, 0);
        settingsGrid.Controls.Add(createBackup, 0, 10);

        var summaryGrid = new BufferedTableLayoutPanel
        {
            Dock = DockStyle.Fill,
            ColumnCount = 1,
            RowCount = 7,
            BackColor = Palette.Card,
            Margin = Padding.Empty
        };
        summaryGrid.RowStyles.Add(new RowStyle(SizeType.Absolute, 54));
        summaryGrid.RowStyles.Add(new RowStyle(SizeType.Absolute, 32));
        summaryGrid.RowStyles.Add(new RowStyle(SizeType.Absolute, 52));
        summaryGrid.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
        summaryGrid.RowStyles.Add(new RowStyle(SizeType.Absolute, 32));
        summaryGrid.RowStyles.Add(new RowStyle(SizeType.Absolute, 54));
        summaryGrid.RowStyles.Add(new RowStyle(SizeType.Absolute, 46));
        summary.Controls.Add(summaryGrid);
        summaryGrid.Controls.Add(SectionTitle("Preview", "Compare current and pending settings."), 0, 0);
        current.Dock = DockStyle.Fill; current.ForeColor = Palette.Muted;
        current.Font = new Font(Font.FontFamily, 10F); current.AccessibleName = "Current display settings";
        current.TextAlign = ContentAlignment.MiddleLeft;
        summaryGrid.Controls.Add(current, 0, 1);
        pending.Dock = DockStyle.Fill; pending.Font = new Font(Font.FontFamily, 15, FontStyle.Bold);
        pending.ForeColor = Palette.Text; pending.AccessibleName = "Pending display settings";
        pending.TextAlign = ContentAlignment.MiddleLeft;
        summaryGrid.Controls.Add(pending, 0, 2);
        preview.Dock = DockStyle.Fill; preview.Margin = new Padding(0, 10, 0, 10);
        preview.BackColor = Palette.Card; preview.AccessibleName = "Resolution aspect preview";
        preview.Paint += PreviewPaint;
        summaryGrid.Controls.Add(preview, 0, 3);
        summaryGrid.Controls.Add(new Label
        {
            Text = "LIVE ASPECT PREVIEW",
            Dock = DockStyle.Fill,
            TextAlign = ContentAlignment.MiddleCenter,
            ForeColor = Palette.Muted,
            Font = new Font(Font.FontFamily, 9, FontStyle.Bold)
        }, 0, 4);
        summaryGrid.Controls.Add(new Label
        {
            Text = "16:9 fills the screen; narrower modes show pillarboxing.",
            Dock = DockStyle.Fill,
            ForeColor = Palette.Muted,
            BackColor = Palette.Input,
            Padding = new Padding(12, 12, 12, 0)
        }, 0, 5);
        var manageBackups = new Button
        {
            Text = "Manage backups",
            AutoSize = false,
            Size = new Size(132, 38),
            Anchor = AnchorStyles.Top | AnchorStyles.Left,
            Margin = new Padding(8, 4, 0, 0),
            AccessibleName = "Browse and restore configuration backups"
        };
        StyleButton(manageBackups, false);
        manageBackups.Click += (_, _) => ShowBackups();
        var diagnostics = new Button
        {
            Text = "Diagnostics",
            AutoSize = false,
            Size = new Size(112, 38),
            Margin = new Padding(8, 4, 0, 0),
            AccessibleName = "Open privacy-safe diagnostics"
        };
        StyleButton(diagnostics, false);
        diagnostics.Click += (_, _) => ShowDiagnostics();
        var tools = new BufferedFlowLayoutPanel
        {
            Dock = DockStyle.Fill,
            FlowDirection = FlowDirection.LeftToRight,
            WrapContents = false,
            BackColor = Palette.Card,
            Margin = Padding.Empty
        };
        tools.Controls.Add(manageBackups); tools.Controls.Add(diagnostics);
        summaryGrid.Controls.Add(tools, 0, 6);

        var footer = new BufferedTableLayoutPanel
        {
            Dock = DockStyle.Fill,
            ColumnCount = 2,
            RowCount = 1,
            BackColor = Palette.Window,
            Margin = Padding.Empty
        };
        footer.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        footer.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        status.Dock = DockStyle.Fill;
        status.ForeColor = Palette.Muted; status.Padding = new Padding(2, 18, 0, 0); status.AccessibleName = "Application status";
        footer.Controls.Add(status, 0, 0);
        var actions = new BufferedFlowLayoutPanel
        {
            AutoSize = true,
            Dock = DockStyle.Fill,
            FlowDirection = FlowDirection.LeftToRight,
            WrapContents = false,
            BackColor = Palette.Window,
            Padding = new Padding(0, 8, 0, 0)
        };
        StyleButton(reset, false);
        reset.Text = "Reset"; reset.Size = new Size(104, 42); reset.Enabled = false; reset.AccessibleName = "Reset pending changes";
        reset.Click += (_, _) => ResetPending();
        StyleButton(apply, true);
        apply.Text = "Apply changes"; apply.Size = new Size(148, 42); apply.Enabled = false; apply.AccessibleName = "Apply pending changes";
        apply.Click += (_, _) => Apply();
        actions.Controls.Add(reset); actions.Controls.Add(apply);
        footer.Controls.Add(actions, 1, 0);
        root.Controls.Add(footer, 0, 2);
        AcceptButton = apply;
        UpdateResponsiveLayout();
    }

    private void InitializeData()
    {
        preferences = preferenceService.Read(settingsPath);
        if (preferences.Warning is not null) SetStatus(preferences.Warning, true);
        RefreshDisplays();
        loading = true;
        aspect.SelectedIndex = 0;
        var initialPreset = ResolutionCatalog.RecommendedPreset(0);
        PopulatePresetChoices(0, initialPreset.Width, initialPreset.Height);
        customWidth.Text = initialPreset.Width.ToString();
        customHeight.Text = initialPreset.Height.ToString();
        loading = false;
        RefreshAccounts();
    }

    private void RefreshAccounts()
    {
        var priorPath = selectedPath;
        loading = true;
        accounts.Items.Clear();
        IReadOnlyList<string> roots = steamRoot is null
            ? steam.GetRoots()
            : Directory.Exists(steamRoot) ? [Path.GetFullPath(steamRoot)] : [];
        var discoveredAccounts = steam.GetAccounts(roots);
        discoveredRootCount = roots.Count;
        discoveredAccountCount = discoveredAccounts.Count;
        foreach (var account in discoveredAccounts) accounts.Items.Add(account);
        foreach (var path in preferences.RecentConfigPaths.Where(File.Exists))
            if (!accounts.Items.Cast<object>().OfType<SteamAccount>().Any(x => string.Equals(x.ConfigPath, path, StringComparison.OrdinalIgnoreCase)))
                accounts.Items.Add(new SteamAccount($"Custom file  -  {path}", "", 0, "Custom file", null, false, path, true, File.GetLastWriteTime(path)));
        accounts.DisplayMember = nameof(SteamAccount.DisplayName);
        var selection = accounts.Items.Cast<object>().OfType<SteamAccount>().ToList().FindIndex(x =>
            string.Equals(x.ConfigPath, priorPath, StringComparison.OrdinalIgnoreCase) ||
            (priorPath is null && x.AccountId == preferences.LastAccountId && x.HasConfig));
        if (selection < 0) selection = accounts.Items.Cast<object>().OfType<SteamAccount>().ToList().FindIndex(x => x.HasConfig);
        if (selection >= 0) accounts.SelectedIndex = selection;
        loading = false;
        AccountChanged();
        SetStatus(accounts.Items.Count == 0 ? "No Steam accounts found. Use Browse to select cs2_video.txt." : "Steam accounts refreshed.");
    }

    private void AccountChanged()
    {
        if (loading || accounts.SelectedItem is not SteamAccount account) return;
        if (!account.HasConfig) { LoadPath(null); SetStatus("This account does not have a CS2 video configuration.", true); return; }
        LoadPath(account.ConfigPath);
    }

    private void Browse()
    {
        using var dialog = new OpenFileDialog
        {
            Title = "Select the CS2 video configuration",
            Filter = "CS2 video configuration (cs2_video.txt)|cs2_video.txt|Text files (*.txt)|*.txt",
            CheckFileExists = true
        };
        if (dialog.ShowDialog(this) != DialogResult.OK) return;
        try
        {
            _ = configs.Read(dialog.FileName);
            preferences.RecentConfigPaths = preferenceService.AddRecent(preferences.RecentConfigPaths, dialog.FileName);
            loading = true;
            accounts.Items.Add(new SteamAccount($"Custom file  -  {dialog.FileName}", "", 0, "Custom file", null, false,
                dialog.FileName, true, File.GetLastWriteTime(dialog.FileName)));
            accounts.SelectedIndex = accounts.Items.Count - 1;
            loading = false;
            LoadPath(dialog.FileName);
        }
        catch (Exception ex) { ShowError(ex.Message); }
    }

    private void LoadPath(string? path)
    {
        selectedPath = path;
        filePath.Text = path ?? "No valid configuration selected.";
        original = null;
        if (path is null) { current.Text = pending.Text = "—"; apply.Enabled = reset.Enabled = false; preview.Invalidate(); return; }
        try
        {
            original = configs.Read(path);
            ResetPending();
            var account = accounts.SelectedItem as SteamAccount;
            preferences = preferenceService.Save(account?.AccountId, preferenceService.AddRecent(preferences.RecentConfigPaths, path), settingsPath);
            SetStatus("Configuration loaded.");
        }
        catch (Exception ex) { ShowError(ex.Message); }
    }

    private void ResetPending()
    {
        if (original is null) return;
        loading = true;
        aspect.SelectedIndex = original.AspectMode;
        PopulatePresetChoices(original.AspectMode, original.Width, original.Height);
        customWidth.Text = original.Width.ToString();
        customHeight.Text = original.Height.ToString();
        loading = false;
        PendingChanged();
    }

    private void PresetChanged()
    {
        if (loading) return;
        if (presets.SelectedIndex == 0)
        {
            SetCustomDimensionsVisible(true);
            if (original is not null)
            {
                loading = true;
                customWidth.Text = original.Width.ToString();
                customHeight.Text = original.Height.ToString();
                loading = false;
            }
            PendingChanged();
            return;
        }
        if (presets.SelectedIndex < 0) { SetCustomDimensionsVisible(false); PendingChanged(); return; }
        SetCustomDimensionsVisible(false);
        var item = displayedPresets[presets.SelectedIndex - 1];
        loading = true;
        customWidth.Text = item.Width.ToString();
        customHeight.Text = item.Height.ToString();
        aspect.SelectedIndex = item.Mode;
        loading = false;
        PendingChanged();
    }

    private void AspectChanged()
    {
        if (loading || aspect.SelectedIndex < 0) return;
        var recommended = ResolutionCatalog.RecommendedPreset(aspect.SelectedIndex);
        loading = true;
        PopulatePresetChoices(aspect.SelectedIndex, recommended.Width, recommended.Height);
        customWidth.Text = recommended.Width.ToString();
        customHeight.Text = recommended.Height.ToString();
        loading = false;
        PendingChanged();
    }

    private void RefreshDisplays()
    {
        var prior = (displays.SelectedItem as DisplayInfo)?.DeviceName;
        try { detectedDisplays = displayProvider.GetDisplays(); }
        catch { detectedDisplays = []; }
        loading = true;
        displays.Items.Clear();
        foreach (var display in detectedDisplays) displays.Items.Add(display);
        displays.DisplayMember = nameof(DisplayInfo.Label);
        if (displays.Items.Count == 0) displays.Items.Add("No displays reported by Windows");
        var index = detectedDisplays.ToList().FindIndex(x => string.Equals(x.DeviceName, prior, StringComparison.OrdinalIgnoreCase));
        if (index < 0) index = detectedDisplays.ToList().FindIndex(x => x.IsPrimary);
        displays.SelectedIndex = index >= 0 ? index : 0;
        loading = false;
        DisplayChanged();
    }

    private void DisplayChanged()
    {
        if (loading) return;
        supportedModes = displays.SelectedItem is DisplayInfo display
            ? display.Modes.Select(x => (x.Width, x.Height)).ToHashSet()
            : [];
        var mode = aspect.SelectedIndex >= 0 ? aspect.SelectedIndex : 0;
        var width = int.TryParse(customWidth.Text, out var w) ? w : (int?)null;
        var height = int.TryParse(customHeight.Text, out var h) ? h : (int?)null;
        loading = true;
        PopulatePresetChoices(mode, width, height);
        loading = false;
        PendingChanged();
    }

    private void PopulatePresetChoices(int mode, int? width = null, int? height = null)
    {
        var supported = supportedModes
            .Select(x => ResolutionCatalog.Parse($"{x.Width}x{x.Height}"))
            .Where(x => x.Mode == mode)
            .OrderBy(x => x.Width).ThenBy(x => x.Height);
        var catalogOnly = ResolutionCatalog.Presets.Where(x => x.Mode == mode &&
            !supportedModes.Contains((x.Width, x.Height)));
        displayedPresets = supported.Concat(catalogOnly).DistinctBy(x => (x.Width, x.Height)).ToArray();
        presets.Items.Clear();
        presets.Items.Add("Custom resolution");
        foreach (var resolution in displayedPresets)
            presets.Items.Add($"{resolution.Display}  ({resolution.Ratio})  —  {(supportedModes.Contains((resolution.Width, resolution.Height)) ? "available" : "not reported")}");
        var matchingIndex = displayedPresets.ToList().FindIndex(x => x.Width == width && x.Height == height);
        presets.SelectedIndex = matchingIndex + 1;
        SetCustomDimensionsVisible(presets.SelectedIndex == 0);
    }

    private void SetCustomDimensionsVisible(bool visible)
    {
        if (settingsGrid is null || customDimensions is null) return;
        settingsGrid.SuspendLayout();
        customDimensions.Visible = visible;
        customWidth.TabStop = visible;
        customHeight.TabStop = visible;
        settingsGrid.RowStyles[8].Height = visible ? 86 : 0;
        settingsGrid.ResumeLayout(true);
    }

    private void PendingChanged()
    {
        if (loading) return;
        if (original is null) { apply.Enabled = reset.Enabled = false; validation.Text = ""; availability.Text = ""; return; }
        current.Text = $"Current:  {original.Width} × {original.Height}  ·  {ResolutionCatalog.ModeName(original.AspectMode, original.Width, original.Height)}";
        try
        {
            var resolution = ResolutionCatalog.Parse($"{customWidth.Text}x{customHeight.Text}",
                aspect.SelectedIndex >= 0 ? aspect.SelectedIndex : null);
            pending.Text = $"Pending:  {resolution.Width} × {resolution.Height}  ·  {ResolutionCatalog.ModeName(resolution.Mode, resolution.Width, resolution.Height)}";
            var changed = original != new VideoConfigState(resolution.Width, resolution.Height, resolution.Mode);
            apply.Enabled = reset.Enabled = changed;
            validation.Text = "";
            availability.Text = supportedModes.Count == 0 ? "Display modes unavailable"
                : supportedModes.Contains((resolution.Width, resolution.Height)) ? "✓ Reported for selected display"
                : "Not reported; custom use is still allowed";
            availability.ForeColor = supportedModes.Contains((resolution.Width, resolution.Height)) ? Palette.Accent : Palette.Muted;
            status.Text = changed ? "Review the pending settings, then apply the change." : "No pending changes.";
        }
        catch (Exception ex)
        {
            pending.Text = "Pending: invalid custom resolution";
            status.Text = ex.Message;
            validation.Text = ex.Message;
            availability.Text = "";
            apply.Enabled = false; reset.Enabled = true;
        }
        preview.Invalidate();
    }

    private Resolution PendingResolution() => ResolutionCatalog.Parse($"{customWidth.Text}x{customHeight.Text}", aspect.SelectedIndex);

    private void Apply()
    {
        if (selectedPath is null) return;
        try
        {
            var result = configs.Update(selectedPath, PendingResolution(), createBackup.Checked);
            original = configs.Read(selectedPath);
            ResetPending();
            SetStatus(result.Changed
                ? result.BackupPath is null ? "Changes applied." : $"Changes applied. Backup: {Path.GetFileName(result.BackupPath)}"
                : "The configuration already has these settings.");
        }
        catch (Exception ex) { ShowError(ex.Message); }
    }

    private void ShowBackups()
    {
        if (selectedPath is null) { SetStatus("Select a configuration first.", true); return; }
        var backups = configs.GetBackups(selectedPath);
        using var dialog = new BufferedForm
        {
            Text = "Configuration backups",
            Size = new Size(680, 430),
            StartPosition = FormStartPosition.CenterParent,
            BackColor = Palette.Window,
            ForeColor = Palette.Text,
            Font = Font,
            MinimizeBox = false,
            MaximizeBox = false,
            Padding = new Padding(18)
        };
        dialog.Shown += (_, _) => DarkTitleBar.Apply(dialog.Handle);
        var list = new ListBox
        {
            Dock = DockStyle.Fill,
            DisplayMember = nameof(BackupInfo.Path),
            AccessibleName = "Available backups",
            BackColor = Palette.Input,
            ForeColor = Palette.Text,
            BorderStyle = BorderStyle.FixedSingle,
            Font = new Font(Font.FontFamily, 10),
            ItemHeight = 30
        };
        foreach (var item in backups) list.Items.Add(item);
        var details = new Label
        {
            Dock = DockStyle.Bottom,
            Height = 60,
            Padding = new Padding(10, 14, 10, 0),
            ForeColor = Palette.Muted,
            BackColor = Palette.Window
        };
        list.SelectedIndexChanged += (_, _) =>
        {
            if (list.SelectedItem is BackupInfo item)
                details.Text = $"{item.Created:G}  ·  {item.State.Width} × {item.State.Height}  ·  {ResolutionCatalog.ModeName(item.State.AspectMode, item.State.Width, item.State.Height)}";
        };
        var restore = Button("Restore selected", (_, _) =>
        {
            if (list.SelectedItem is not BackupInfo item) return;
            if (MessageBox.Show(dialog, "Restore this backup? A rollback backup of the active file will be created.",
                "Confirm restore", MessageBoxButtons.YesNo, MessageBoxIcon.Warning) != DialogResult.Yes) return;
            try { configs.Restore(selectedPath, item.Path); dialog.DialogResult = DialogResult.OK; }
            catch (Exception ex) { MessageBox.Show(dialog, ex.Message, "Restore failed", MessageBoxButtons.OK, MessageBoxIcon.Error); }
        }, "Restore selected backup");
        restore.Dock = DockStyle.Bottom;
        restore.Height = 44;
        restore.Margin = new Padding(0, 8, 0, 0);
        dialog.Controls.Add(list); dialog.Controls.Add(details); dialog.Controls.Add(restore);
        if (backups.Count == 0) details.Text = "No editor backups were found.";
        if (dialog.ShowDialog(this) == DialogResult.OK) { LoadPath(selectedPath); SetStatus("Backup restored successfully."); }
    }

    private void ShowDiagnostics()
    {
        var version = typeof(MainForm).Assembly.GetName().Version?.ToString(3) ?? "unknown";
        var report = diagnosticService.Create(version, detectedDisplays, discoveredRootCount, discoveredAccountCount, selectedPath);
        var summary = DiagnosticService.ToSummary(report);
        var json = DiagnosticService.ToJson(report);
        using var dialog = new BufferedForm
        {
            Text = "Privacy-safe diagnostics",
            Size = new Size(720, 540),
            StartPosition = FormStartPosition.CenterParent,
            BackColor = Palette.Window,
            ForeColor = Palette.Text,
            Font = Font,
            MinimizeBox = false,
            MaximizeBox = false,
            Padding = new Padding(20)
        };
        dialog.Shown += (_, _) => DarkTitleBar.Apply(dialog.Handle);
        var text = new TextBox
        {
            Multiline = true,
            ReadOnly = true,
            ScrollBars = ScrollBars.Vertical,
            Dock = DockStyle.Fill,
            BackColor = Palette.Input,
            ForeColor = Palette.Text,
            BorderStyle = BorderStyle.FixedSingle,
            Text = summary,
            AccessibleName = "Privacy-safe diagnostic summary"
        };
        var buttons = new BufferedFlowLayoutPanel
        {
            Dock = DockStyle.Bottom,
            Height = 52,
            FlowDirection = FlowDirection.RightToLeft,
            BackColor = Palette.Window,
            Padding = new Padding(0, 8, 0, 0)
        };
        buttons.Controls.Add(Button("Export JSON", (_, _) =>
        {
            try
            {
                using var save = new SaveFileDialog { Filter = "JSON report (*.json)|*.json", FileName = "cs2-resedit-diagnostics.json" };
                if (save.ShowDialog(dialog) == DialogResult.OK)
                    File.WriteAllText(save.FileName, json, new System.Text.UTF8Encoding(false));
            }
            catch (Exception ex) { MessageBox.Show(dialog, ex.Message, "Export failed", MessageBoxButtons.OK, MessageBoxIcon.Error); }
        }, "Export privacy-safe diagnostics as JSON"));
        buttons.Controls.Add(Button("Copy", (_, _) =>
        {
            try { Clipboard.SetText(summary); }
            catch (Exception ex) { MessageBox.Show(dialog, ex.Message, "Copy failed", MessageBoxButtons.OK, MessageBoxIcon.Error); }
        }, "Copy privacy-safe diagnostic summary"));
        dialog.Controls.Add(text); dialog.Controls.Add(buttons);
        dialog.ShowDialog(this);
    }

    private void PreviewPaint(object? sender, PaintEventArgs e)
    {
        var width = int.TryParse(customWidth.Text, out var w) ? w : original?.Width ?? 16;
        var height = int.TryParse(customHeight.Text, out var h) ? h : original?.Height ?? 9;
        var viewport = CalculatePreviewViewport(preview.ClientRectangle);
        using var screen = new SolidBrush(Palette.Preview);
        using var screenBorder = new Pen(Palette.Border, 1);
        e.Graphics.FillRectangle(screen, viewport);
        e.Graphics.DrawRectangle(screenBorder, viewport.X, viewport.Y, viewport.Width, viewport.Height);
        var rect = CalculatePreviewRectangle(viewport, width, height);
        using var fill = new SolidBrush(Palette.PreviewFill);
        using var pen = new Pen(Palette.Accent, 2);
        e.Graphics.FillRectangle(fill, rect); e.Graphics.DrawRectangle(pen, rect.X, rect.Y, rect.Width, rect.Height);
    }

    private static RectangleF CalculatePreviewViewport(Rectangle bounds)
    {
        if (bounds.Width <= 0 || bounds.Height <= 0) return RectangleF.Empty;
        const float screenAspect = 16F / 9F;
        var viewportWidth = Math.Min(bounds.Width, bounds.Height * screenAspect);
        var viewportHeight = viewportWidth / screenAspect;
        return new RectangleF(
            bounds.Left + (bounds.Width - viewportWidth) / 2F,
            bounds.Top + (bounds.Height - viewportHeight) / 2F,
            viewportWidth,
            viewportHeight);
    }

    private static RectangleF CalculatePreviewRectangle(RectangleF bounds, int width, int height)
    {
        if (bounds.Width <= 0 || bounds.Height <= 0 || width <= 0 || height <= 0)
            return RectangleF.Empty;

        const float widestStandardAspect = 16F / 9F;
        var aspect = (float)width / height;
        var referenceAspect = Math.Max(widestStandardAspect, aspect);
        var sharedHeight = Math.Min(bounds.Height, bounds.Width / referenceAspect);
        var previewWidth = sharedHeight * aspect;
        return new RectangleF(
            bounds.Left + (bounds.Width - previewWidth) / 2F,
            bounds.Top + (bounds.Height - sharedHeight) / 2F,
            previewWidth,
            sharedHeight);
    }

    private void UpdateResponsiveLayout()
    {
        if (contentHost.Width <= 0 || contentHost.Height <= 0) return;
        contentHost.AutoScroll = false;
        if (ClientSize.Width < 1100)
        {
            body.ColumnCount = 1; body.RowCount = 2;
            body.ColumnStyles.Clear(); body.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
            body.RowStyles.Clear();
            body.RowStyles.Add(new RowStyle(SizeType.Absolute, 640));
            body.RowStyles.Add(new RowStyle(SizeType.Absolute, 520));
            body.SetCellPosition(body.Controls[1], new TableLayoutPanelCellPosition(0, 1));
            body.Size = new Size(Math.Max(700, contentHost.ClientSize.Width - 12), 1160);
            body.Location = new Point(6, 0);
            contentHost.AutoScroll = true;
        }
        else
        {
            body.ColumnCount = 2; body.RowCount = 1;
            body.ColumnStyles.Clear();
            body.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 61));
            body.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 39));
            body.RowStyles.Clear();
            body.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
            body.SetCellPosition(body.Controls[1], new TableLayoutPanelCellPosition(1, 0));
            var width = Math.Min(1180, contentHost.ClientSize.Width - 12);
            var height = Math.Min(650, contentHost.ClientSize.Height - 8);
            body.Size = new Size(width, Math.Max(620, height));
            body.Location = new Point((contentHost.ClientSize.Width - width) / 2, Math.Max(0, (contentHost.ClientSize.Height - body.Height) / 2));
        }
    }

    private Panel Card() => new RoundedPanel
    {
        Dock = DockStyle.Fill,
        Padding = new Padding(24),
        Margin = new Padding(7),
        BackColor = Palette.Card,
        BorderColor = Palette.Border,
        CornerRadius = 14
    };

    private Control SectionTitle(string title, string subtitle)
    {
        var panel = new BufferedFlowLayoutPanel
        {
            Dock = DockStyle.Fill,
            FlowDirection = FlowDirection.TopDown,
            WrapContents = false,
            BackColor = Palette.Card,
            Margin = Padding.Empty
        };
        panel.Controls.Add(new Label
        {
            Text = title,
            AutoSize = true,
            ForeColor = Palette.Text,
            Font = new Font(Font.FontFamily, 13, FontStyle.Bold)
        });
        panel.Controls.Add(new Label
        {
            Text = subtitle,
            AutoSize = true,
            ForeColor = Palette.Muted,
            Font = new Font(Font.FontFamily, 10),
            Margin = new Padding(0, 2, 0, 0)
        });
        return panel;
    }

    private Label FieldLabel(string text) => new()
    {
        Text = text,
        Dock = DockStyle.Fill,
        ForeColor = Palette.Muted,
        Font = new Font(Font.FontFamily, 9, FontStyle.Bold),
        TextAlign = ContentAlignment.BottomLeft
    };

    private Control Labeled(string label, Control control)
    {
        var panel = new BufferedTableLayoutPanel
        {
            Dock = DockStyle.Fill,
            ColumnCount = 1,
            RowCount = 2,
            BackColor = Palette.Card,
            Margin = new Padding(0, 0, 12, 0)
        };
        panel.RowStyles.Add(new RowStyle(SizeType.Absolute, 24));
        panel.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
        panel.Controls.Add(FieldLabel(label), 0, 0);
        panel.Controls.Add(control, 0, 1);
        return panel;
    }

    private static void ConfigureCombo(ComboBox combo, string name)
    {
        combo.DropDownStyle = ComboBoxStyle.DropDownList;
        combo.AccessibleName = name;
        combo.Margin = Padding.Empty;
        combo.BackColor = Palette.Input;
        combo.ForeColor = Palette.Text;
        combo.FlatStyle = FlatStyle.Flat;
        combo.DrawMode = DrawMode.OwnerDrawFixed;
        combo.ItemHeight = 30;
        combo.DrawItem += (_, e) =>
        {
            if (e.Index < 0) return;
            var selected = (e.State & DrawItemState.Selected) != 0;
            using var background = new SolidBrush(selected ? Palette.AccentSoft : Palette.Input);
            e.Graphics.FillRectangle(background, e.Bounds);
            var textBounds = new Rectangle(e.Bounds.X + 10, e.Bounds.Y, e.Bounds.Width - 16, e.Bounds.Height);
            TextRenderer.DrawText(
                e.Graphics,
                combo.GetItemText(combo.Items[e.Index]),
                combo.Font,
                textBounds,
                Palette.Text,
                TextFormatFlags.VerticalCenter |
                TextFormatFlags.SingleLine |
                TextFormatFlags.EndEllipsis |
                TextFormatFlags.NoPrefix |
                TextFormatFlags.PreserveGraphicsClipping);
        };
    }

    private static void ConfigureText(TextBox box, string name)
    {
        box.AccessibleName = name;
        box.Margin = Padding.Empty;
        box.Dock = DockStyle.Fill;
        box.BackColor = Palette.Input;
        box.ForeColor = Palette.Text;
        box.BorderStyle = BorderStyle.FixedSingle;
        box.Font = new Font("Segoe UI", 11);
        box.Padding = new Padding(8);
    }

    private static Button Button(string text, EventHandler click, string accessibleName)
    {
        var button = new Button
        {
            Text = text,
            AutoSize = true,
            MinimumSize = new Size(84, 36),
            AccessibleName = accessibleName,
            Margin = new Padding(8, 0, 0, 0)
        };
        StyleButton(button, false);
        button.Click += click; return button;
    }

    private static void StyleButton(Button button, bool primary)
    {
        button.FlatStyle = FlatStyle.Flat;
        button.UseVisualStyleBackColor = false;
        button.FlatAppearance.BorderSize = 1;
        button.FlatAppearance.BorderColor = primary ? Palette.Accent : Palette.BorderStrong;
        button.FlatAppearance.MouseOverBackColor = primary ? Palette.AccentHover : Palette.CardHover;
        button.FlatAppearance.MouseDownBackColor = primary ? Palette.AccentPressed : Palette.Input;
        button.BackColor = primary ? Palette.Accent : Palette.Card;
        button.ForeColor = primary ? Palette.AccentText : Palette.Text;
        button.Cursor = Cursors.Hand;
        button.Font = new Font("Segoe UI", 9.5F, FontStyle.Bold);
        button.GotFocus += (_, _) =>
        {
            button.FlatAppearance.BorderSize = 2;
            button.FlatAppearance.BorderColor = Palette.Accent;
            button.Invalidate();
        };
        button.LostFocus += (_, _) =>
        {
            button.FlatAppearance.BorderSize = 1;
            button.FlatAppearance.BorderColor = primary ? Palette.Accent : Palette.BorderStrong;
            button.Invalidate();
        };
        button.EnabledChanged += (_, _) =>
        {
            if (!button.Enabled)
            {
                button.BackColor = Palette.Card;
                button.ForeColor = Palette.Muted;
                button.FlatAppearance.BorderColor = Palette.BorderStrong;
            }
            else
            {
                button.BackColor = primary ? Palette.Accent : Palette.Card;
                button.ForeColor = primary ? Palette.AccentText : Palette.Text;
                button.FlatAppearance.BorderColor = primary ? Palette.Accent : Palette.BorderStrong;
            }
            button.Invalidate();
        };
    }

    private void SetStatus(string message, bool warning = false) { status.Text = message; status.ForeColor = warning ? Palette.Warning : Palette.Muted; }
    private void ShowError(string message) { SetStatus(message, true); MessageBox.Show(this, message, "CS2 ResEdit", MessageBoxButtons.OK, MessageBoxIcon.Error); }
    private void ApplyDarkTitleBar() => DarkTitleBar.Apply(Handle);
    private void TryLoadIcon()
    {
        try
        {
            using var executableIcon = Icon.ExtractAssociatedIcon(Application.ExecutablePath);
            if (executableIcon is not null)
                Icon = (Icon)executableIcon.Clone();
        }
        catch (Exception) { }
    }

    protected override void WndProc(ref Message message)
    {
        base.WndProc(ref message);
        if (message.Msg == 0x007E && IsHandleCreated && !IsDisposed && !Disposing)
            BeginInvoke((Action)RefreshDisplays);
    }
}

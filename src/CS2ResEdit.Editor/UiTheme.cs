using System.Drawing.Drawing2D;
using System.Runtime.InteropServices;
using System.ComponentModel;

namespace Softhe.CS2ResEdit.Editor;

public sealed record ThemePalette(
    Color Window,
    Color Card,
    Color CardHover,
    Color Input,
    Color Preview,
    Color PreviewFill,
    Color Border,
    Color BorderStrong,
    Color Text,
    Color Muted,
    Color Disabled,
    Color Accent,
    Color AccentText,
    Color AccentHover,
    Color AccentPressed,
    Color AccentSoft,
    Color Warning)
{
    public static ThemePalette Create(bool highContrast) => highContrast
        ? new(
            SystemColors.Window,
            SystemColors.Control,
            SystemColors.ControlLight,
            SystemColors.Window,
            SystemColors.Window,
            SystemColors.Highlight,
            SystemColors.WindowText,
            SystemColors.Highlight,
            SystemColors.WindowText,
            SystemColors.WindowText,
            SystemColors.GrayText,
            SystemColors.Highlight,
            SystemColors.HighlightText,
            SystemColors.HotTrack,
            SystemColors.Highlight,
            SystemColors.Control,
            SystemColors.Highlight)
        : new(
            Color.FromArgb(18, 20, 22),
            Color.FromArgb(28, 32, 35),
            Color.FromArgb(41, 46, 50),
            Color.FromArgb(21, 25, 28),
            Color.FromArgb(13, 15, 17),
            Color.FromArgb(90, 64, 34),
            Color.FromArgb(60, 67, 72),
            Color.FromArgb(100, 111, 118),
            Color.FromArgb(238, 234, 228),
            Color.FromArgb(187, 179, 168),
            Color.FromArgb(137, 130, 122),
            Color.FromArgb(248, 157, 28),
            Color.FromArgb(36, 24, 10),
            Color.FromArgb(255, 180, 71),
            Color.FromArgb(216, 127, 5),
            Color.FromArgb(61, 45, 25),
            Color.FromArgb(241, 192, 106));
}

internal static class Palette
{
    public static ThemePalette Current { get; } = ThemePalette.Create(SystemInformation.HighContrast);
    public static Color Window => Current.Window;
    public static Color Card => Current.Card;
    public static Color CardHover => Current.CardHover;
    public static Color Input => Current.Input;
    public static Color Preview => Current.Preview;
    public static Color PreviewFill => Current.PreviewFill;
    public static Color Border => Current.Border;
    public static Color BorderStrong => Current.BorderStrong;
    public static Color Text => Current.Text;
    public static Color Muted => Current.Muted;
    public static Color Disabled => Current.Disabled;
    public static Color Accent => Current.Accent;
    public static Color AccentText => Current.AccentText;
    public static Color AccentHover => Current.AccentHover;
    public static Color AccentPressed => Current.AccentPressed;
    public static Color AccentSoft => Current.AccentSoft;
    public static Color Warning => Current.Warning;
}

public class BufferedForm : Form
{
    public BufferedForm()
    {
        DoubleBuffered = true;
        SetStyle(ControlStyles.AllPaintingInWmPaint |
            ControlStyles.OptimizedDoubleBuffer |
            ControlStyles.ResizeRedraw, true);
    }
}

internal sealed class BufferedTableLayoutPanel : TableLayoutPanel
{
    public BufferedTableLayoutPanel()
    {
        DoubleBuffered = true;
        SetStyle(ControlStyles.AllPaintingInWmPaint |
            ControlStyles.OptimizedDoubleBuffer |
            ControlStyles.ResizeRedraw, true);
    }
}

internal sealed class BufferedFlowLayoutPanel : FlowLayoutPanel
{
    public BufferedFlowLayoutPanel()
    {
        DoubleBuffered = true;
        SetStyle(ControlStyles.AllPaintingInWmPaint |
            ControlStyles.OptimizedDoubleBuffer |
            ControlStyles.ResizeRedraw, true);
    }
}

internal sealed class RoundedPanel : Panel
{
    [DesignerSerializationVisibility(DesignerSerializationVisibility.Hidden)]
    public int CornerRadius { get; set; } = 12;

    [DesignerSerializationVisibility(DesignerSerializationVisibility.Hidden)]
    public Color BorderColor { get; set; } = Palette.Border;

    public RoundedPanel()
    {
        SetStyle(ControlStyles.AllPaintingInWmPaint |
            ControlStyles.OptimizedDoubleBuffer |
            ControlStyles.ResizeRedraw |
            ControlStyles.SupportsTransparentBackColor |
            ControlStyles.UserPaint, true);
    }

    protected override void OnPaintBackground(PaintEventArgs e)
    {
        e.Graphics.Clear(Parent?.BackColor ?? Palette.Window);
    }

    protected override void OnPaint(PaintEventArgs e)
    {
        e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
        e.Graphics.PixelOffsetMode = PixelOffsetMode.Half;
        e.Graphics.CompositingQuality = CompositingQuality.HighQuality;
        var bounds = new RectangleF(0.5F, 0.5F, Width - 1F, Height - 1F);
        using var path = Rounded(bounds, CornerRadius);
        using var background = new SolidBrush(BackColor);
        using var border = new Pen(BorderColor, 1F);
        e.Graphics.FillPath(background, path);
        e.Graphics.DrawPath(border, path);
    }

    private static GraphicsPath Rounded(RectangleF bounds, float radius)
    {
        var diameter = radius * 2;
        var path = new GraphicsPath();
        path.AddArc(bounds.Left, bounds.Top, diameter, diameter, 180, 90);
        path.AddArc(bounds.Right - diameter, bounds.Top, diameter, diameter, 270, 90);
        path.AddArc(bounds.Right - diameter, bounds.Bottom - diameter, diameter, diameter, 0, 90);
        path.AddArc(bounds.Left, bounds.Bottom - diameter, diameter, diameter, 90, 90);
        path.CloseFigure();
        return path;
    }
}

internal sealed class DarkComboBox : ComboBox
{
    private const int WmPaint = 0x000F;
    private const int WmEraseBackground = 0x0014;
    private const int WmPrintClient = 0x0318;
    private const int WmLeftButtonDown = 0x0201;
    private const int WmLeftButtonUp = 0x0202;
    private const int WmLeftButtonDoubleClick = 0x0203;
    private const int ComboShowDropDown = 0x014F;

    private bool HasMultipleChoices => Items.Count > 1;

    protected override void WndProc(ref Message message)
    {
        if (DropDownStyle != ComboBoxStyle.DropDownList)
        {
            base.WndProc(ref message);
            return;
        }

        if (message.Msg == WmEraseBackground)
        {
            message.Result = new IntPtr(1);
            return;
        }

        if (message.Msg is WmLeftButtonDown or WmLeftButtonDoubleClick)
        {
            Focus();
            if (HasMultipleChoices) DroppedDown = !DroppedDown;
            Invalidate();
            message.Result = IntPtr.Zero;
            return;
        }

        if (message.Msg == WmLeftButtonUp)
        {
            Invalidate();
            message.Result = IntPtr.Zero;
            return;
        }

        if (message.Msg == ComboShowDropDown && message.WParam != IntPtr.Zero && !HasMultipleChoices)
        {
            message.Result = IntPtr.Zero;
            return;
        }

        if (message.Msg == WmPaint)
        {
            var paint = new PaintStruct();
            var deviceContext = BeginPaint(Handle, ref paint);
            try
            {
                using var target = Graphics.FromHdc(deviceContext);
                RenderBuffered(target);
            }
            finally { _ = EndPaint(Handle, ref paint); }
            message.Result = IntPtr.Zero;
            return;
        }

        if (message.Msg == WmPrintClient)
        {
            using var target = Graphics.FromHdc(message.WParam);
            RenderBuffered(target);
            message.Result = IntPtr.Zero;
            return;
        }

        base.WndProc(ref message);
    }

    private void RenderBuffered(Graphics target)
    {
        if (ClientSize.Width <= 0 || ClientSize.Height <= 0) return;
        using var buffer = BufferedGraphicsManager.Current.Allocate(target, ClientRectangle);
        DrawSurface(buffer.Graphics);
        buffer.Render(target);
    }

    private void DrawSurface(Graphics graphics)
    {
        using var background = new SolidBrush(Palette.Input);
        using var border = new Pen(Focused ? Palette.Accent : Palette.BorderStrong, Focused ? 2F : 1F);
        using var textBrush = new SolidBrush(Enabled ? Palette.Text : Palette.Disabled);
        graphics.FillRectangle(background, ClientRectangle);
        graphics.DrawRectangle(border, 0, 0, Width - 1, Height - 1);

        if (HasMultipleChoices)
        {
            var arrowArea = new Rectangle(Width - 34, 1, 33, Height - 2);
            var centerX = arrowArea.Left + arrowArea.Width / 2;
            var centerY = arrowArea.Top + arrowArea.Height / 2 + 1;
            using var arrow = new SolidBrush(Palette.Muted);
            graphics.FillPolygon(arrow,
            [
                new Point(centerX - 4, centerY - 2),
                new Point(centerX + 4, centerY - 2),
                new Point(centerX, centerY + 3)
            ]);
        }

        var text = GetItemText(SelectedItem);
        var textBounds = new Rectangle(11, 0, Math.Max(0, Width - (HasMultipleChoices ? 50 : 22)), Height);
        TextRenderer.DrawText(graphics, text, Font, textBounds, textBrush.Color,
            TextFormatFlags.VerticalCenter | TextFormatFlags.EndEllipsis | TextFormatFlags.NoPrefix);
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct PaintStruct
    {
        public IntPtr DeviceContext;
        public int Erase;
        public Rectangle PaintRectangle;
        public int Restore;
        public int IncrementalUpdate;
        public int Reserved1;
        public int Reserved2;
        public int Reserved3;
        public int Reserved4;
        public int Reserved5;
        public int Reserved6;
        public int Reserved7;
        public int Reserved8;
    }

    [DllImport("user32.dll")]
    private static extern IntPtr BeginPaint(IntPtr window, ref PaintStruct paint);

    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool EndPaint(IntPtr window, ref PaintStruct paint);
}

internal sealed class StableFlatButton : Button
{
    private bool hovered;
    private bool pressed;

    public StableFlatButton()
    {
        SetStyle(ControlStyles.AllPaintingInWmPaint |
            ControlStyles.OptimizedDoubleBuffer |
            ControlStyles.ResizeRedraw |
            ControlStyles.UserPaint, true);
    }

    protected override void OnMouseEnter(EventArgs eventargs)
    {
        hovered = true;
        base.OnMouseEnter(eventargs);
        Invalidate();
    }

    protected override void OnMouseLeave(EventArgs eventargs)
    {
        hovered = false;
        pressed = false;
        base.OnMouseLeave(eventargs);
        Invalidate();
    }

    protected override void OnMouseDown(MouseEventArgs eventargs)
    {
        pressed = eventargs.Button == MouseButtons.Left;
        base.OnMouseDown(eventargs);
        Invalidate();
    }

    protected override void OnMouseUp(MouseEventArgs eventargs)
    {
        pressed = false;
        base.OnMouseUp(eventargs);
        Invalidate();
    }

    protected override void OnEnabledChanged(EventArgs eventargs)
    {
        base.OnEnabledChanged(eventargs);
        Invalidate();
    }

    protected override void OnPaint(PaintEventArgs e)
    {
        var background = !Enabled
            ? Palette.Card
            : pressed
                ? FlatAppearance.MouseDownBackColor
                : hovered
                    ? FlatAppearance.MouseOverBackColor
                    : BackColor;
        var border = Focused ? Palette.Accent : FlatAppearance.BorderColor;
        var text = Enabled ? ForeColor : Palette.Muted;

        using var backgroundBrush = new SolidBrush(background);
        using var borderPen = new Pen(border, Focused ? 2F : 1F);
        e.Graphics.FillRectangle(backgroundBrush, ClientRectangle);
        e.Graphics.DrawRectangle(borderPen, 0, 0, Width - 1, Height - 1);
        TextRenderer.DrawText(
            e.Graphics,
            Text,
            Font,
            ClientRectangle,
            text,
            TextFormatFlags.HorizontalCenter |
            TextFormatFlags.VerticalCenter |
            TextFormatFlags.SingleLine |
            TextFormatFlags.EndEllipsis |
            TextFormatFlags.NoPrefix);
    }
}

internal static class DarkTitleBar
{
    [DllImport("dwmapi.dll")]
    private static extern int DwmSetWindowAttribute(IntPtr window, int attribute, ref int value, int size);

    public static void Apply(IntPtr handle)
    {
        var enabled = 1;
        try
        {
            if (DwmSetWindowAttribute(handle, 20, ref enabled, sizeof(int)) != 0)
                _ = DwmSetWindowAttribute(handle, 19, ref enabled, sizeof(int));
        }
        catch (Exception) { }
    }
}

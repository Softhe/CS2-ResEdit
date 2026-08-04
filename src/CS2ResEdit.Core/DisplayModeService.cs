using System.Runtime.InteropServices;

namespace Softhe.CS2ResEdit.Core;

public interface IDisplayModeProvider
{
    IReadOnlyList<DisplayInfo> GetDisplays();
}

public sealed class WindowsDisplayModeProvider : IDisplayModeProvider
{
    public IReadOnlyList<DisplayInfo> GetDisplays()
    {
        var displays = new List<DisplayInfo>();
        for (uint index = 0; ; index++)
        {
            var device = DisplayDevice.Create();
            if (!EnumDisplayDevices(null, index, ref device, 0)) break;
            if ((device.StateFlags & AttachedToDesktop) == 0) continue;

            var modes = new HashSet<DisplayResolution>();
            for (var modeIndex = 0; ; modeIndex++)
            {
                var mode = DevMode.Create();
                if (!EnumDisplaySettings(device.DeviceName, modeIndex, ref mode)) break;
                if (mode.PelsWidth >= 320 && mode.PelsHeight >= 200)
                    modes.Add(new DisplayResolution((int)mode.PelsWidth, (int)mode.PelsHeight));
            }

            var current = DevMode.Create();
            var hasCurrent = EnumDisplaySettings(device.DeviceName, CurrentSettings, ref current);
            displays.Add(new DisplayInfo(
                device.DeviceName,
                string.IsNullOrWhiteSpace(device.DeviceString) ? $"Display {displays.Count + 1}" : device.DeviceString,
                (device.StateFlags & PrimaryDevice) != 0,
                hasCurrent ? (int)current.PelsWidth : 0,
                hasCurrent ? (int)current.PelsHeight : 0,
                modes.OrderBy(x => x.Width).ThenBy(x => x.Height).ToArray()));
        }

        return displays.OrderByDescending(x => x.IsPrimary).ThenBy(x => x.FriendlyName).ToArray();
    }

    private const int CurrentSettings = -1;
    private const int AttachedToDesktop = 0x1;
    private const int PrimaryDevice = 0x4;

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern bool EnumDisplayDevices(string? device, uint index, ref DisplayDevice displayDevice, uint flags);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern bool EnumDisplaySettings(string deviceName, int modeNumber, ref DevMode devMode);

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct DisplayDevice
    {
        public int Size;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string DeviceName;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)] public string DeviceString;
        public int StateFlags;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)] public string DeviceId;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)] public string DeviceKey;

        public static DisplayDevice Create() => new() { Size = Marshal.SizeOf<DisplayDevice>() };
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct DevMode
    {
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string DeviceName;
        public short SpecVersion;
        public short DriverVersion;
        public short Size;
        public short DriverExtra;
        public int Fields;
        public int PositionX;
        public int PositionY;
        public int DisplayOrientation;
        public int DisplayFixedOutput;
        public short Color;
        public short Duplex;
        public short YResolution;
        public short TTOption;
        public short Collate;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string FormName;
        public short LogPixels;
        public int BitsPerPel;
        public uint PelsWidth;
        public uint PelsHeight;
        public int DisplayFlags;
        public int DisplayFrequency;
        public int IcmMethod;
        public int IcmIntent;
        public int MediaType;
        public int DitherType;
        public int Reserved1;
        public int Reserved2;
        public int PanningWidth;
        public int PanningHeight;

        public static DevMode Create() => new() { Size = (short)Marshal.SizeOf<DevMode>() };
    }
}

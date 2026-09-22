using System.Diagnostics;

namespace Softhe.CS2ResEdit.Editor;

public interface IGameProcessProbe
{
    bool IsGameRunning();
}

public sealed class Cs2ProcessProbe : IGameProcessProbe
{
    public bool IsGameRunning()
    {
        try
        {
            return Process.GetProcessesByName("cs2").Length > 0;
        }
        catch (Exception)
        {
            return false;
        }
    }
}

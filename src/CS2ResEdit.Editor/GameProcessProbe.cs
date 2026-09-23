using System.Diagnostics;

namespace Softhe.CS2ResEdit.Editor;

public enum GameRunningState
{
    NotRunning,
    Running,
    Unknown
}

public interface IGameProcessProbe
{
    GameRunningState Check();

    // Back-compat helper for callers/tests that only need a boolean.
    bool IsGameRunning() => Check() == GameRunningState.Running;
}

public sealed class Cs2ProcessProbe : IGameProcessProbe
{
    public GameRunningState Check()
    {
        try
        {
            return Process.GetProcessesByName("cs2").Length > 0
                ? GameRunningState.Running
                : GameRunningState.NotRunning;
        }
        catch (Exception)
        {
            return GameRunningState.Unknown;
        }
    }
}

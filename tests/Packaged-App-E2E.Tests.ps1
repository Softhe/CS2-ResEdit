Describe 'Packaged application end-to-end' {
    It 'builds and runs the isolated release package in every available PowerShell host' {
        $runner = Join-Path $PSScriptRoot '..\build\Invoke-PackagedAppE2E.ps1'
        $result = & $runner

        $result.Passed | Should -BeTrue
        $result.PackageFileCount | Should -Be 5
        @($result.Hosts).Count | Should -BeGreaterOrEqual 1
        @($result.Hosts | Where-Object { -not $_.PresetApplied }).Count | Should -Be 0
        @($result.Hosts | Where-Object { -not $_.DiagnosticsExported }).Count | Should -Be 0
        @($result.Hosts | Where-Object PreferencesCreated).Count | Should -Be 0
        $result.Workspace | Should -BeNullOrEmpty
    }
}

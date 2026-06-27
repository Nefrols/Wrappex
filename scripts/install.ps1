param(
    [string]$InstallDir = "$env:USERPROFILE\.local\bin",
    [switch]$AddToPath
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
Push-Location $repoRoot
try {
    cargo build --release

    $source = Join-Path $repoRoot "target\release\wrappex.exe"
    if (-not (Test-Path -LiteralPath $source)) {
        throw "Release binary was not produced at $source"
    }

    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    $target = Join-Path $InstallDir "wrappex.exe"
    Copy-Item -LiteralPath $source -Destination $target -Force

    if ($AddToPath) {
        $resolvedInstallDir = (Resolve-Path $InstallDir).Path
        $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
        $parts = @($userPath -split ";" | Where-Object { $_ })
        $alreadyPresent = $parts | Where-Object {
            $_.TrimEnd("\") -ieq $resolvedInstallDir.TrimEnd("\")
        }

        if (-not $alreadyPresent) {
            $newPath = if ($userPath) { "$userPath;$resolvedInstallDir" } else { $resolvedInstallDir }
            [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
            Write-Host "Added $resolvedInstallDir to the user PATH. Restart the terminal to use it everywhere."
        }
    }

    Write-Host "Installed Wrappex to $target"
}
finally {
    Pop-Location
}

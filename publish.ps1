param([string]$Message = "Update", [string]$Release = "")
# Commits everything, pushes it to GitHub, and (optionally) publishes a new version.
#   .\publish.ps1 -Message "fixed the dock"
#   .\publish.ps1 -Message "new fishing rods" -Release 0.5.1     (bumps Cargo.toml, tags v0.5.1)
$env:Path = [Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [Environment]::GetEnvironmentVariable("Path","User")
Set-Location $PSScriptRoot
if ($Release) {
    (Get-Content Cargo.toml) -replace '^version = ".*"', "version = `"$Release`"" | Set-Content Cargo.toml
}
git add -A
git commit -m $Message
git push origin main
if ($Release) {
    git tag "v$Release"
    git push origin "v$Release"
    Write-Host "Tagged v$Release - GitHub is now building Island.exe for the auto-updater."
}

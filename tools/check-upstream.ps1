[CmdletBinding()]
param(
  [switch]$PrepareIntegration
)

$ErrorActionPreference = 'Stop'
$repositoryRoot = (& git rev-parse --show-toplevel).Trim()
if (-not $repositoryRoot) {
  throw 'Run this script from inside the Skills Manager Git repository.'
}

Push-Location $repositoryRoot
try {
  $upstreamUrl = (& git remote get-url upstream 2>$null).Trim()
  if (-not $upstreamUrl) {
    throw 'The upstream remote is missing. Add https://github.com/jiweiyeah/skills-manager.git as upstream first.'
  }

  $workingTree = @(& git status --porcelain)
  if ($workingTree.Count -gt 0) {
    throw 'The working tree must be clean before checking upstream updates.'
  }

  & git fetch --no-tags upstream main

  $currentMain = (& git rev-parse main).Trim()
  $upstreamMain = (& git rev-parse upstream/main).Trim()
  $manifestPath = Join-Path $repositoryRoot 'packages/skills-manager-patches/manifest.json'
  $manifest = Get-Content -Raw $manifestPath | ConvertFrom-Json

  Write-Host "upstream: $upstreamUrl"
  Write-Host "current main: $currentMain"
  Write-Host "upstream/main: $upstreamMain"
  Write-Host "manifest reviewed commit: $($manifest.upstream.lastReviewedCommit)"
  Write-Host ''
  & git log --oneline --left-right main...upstream/main
  & git diff --stat main...upstream/main

  if ($PrepareIntegration) {
    $date = Get-Date -Format 'yyyyMMdd'
    $integrationBranch = "integrate/upstream-$date"
    $existingBranch = (& git branch --list $integrationBranch).Trim()
    if ($existingBranch) {
      throw "Integration branch already exists: $integrationBranch"
    }

    & git switch -c $integrationBranch main
    & git merge --no-commit --no-ff upstream/main
    Write-Host "Integration branch prepared: $integrationBranch"
  }
}
finally {
  Pop-Location
}

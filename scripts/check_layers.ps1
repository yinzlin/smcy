# ============================================================================
# check_layers.ps1 - Hexagonal architecture layer guard (long-term CI hook)
#
# See: .trae/documents/Hexagonal-Layer-Rules.md (ADR)
#
# Usage:
#   .\scripts\check_layers.ps1                # all 5 rules
#   .\scripts\check_layers.ps1 -Rule 3        # single rule
#
# Exit: 0 = all pass, non-zero = violation found
# All output is ASCII by design (GBK console safe).
# ============================================================================

param(
    [ValidateRange(1,5)]
    [int]$Rule = 0   # 0 = all
)

$ErrorActionPreference = "Stop"
$script:RepoRoot = Split-Path -Parent $PSScriptRoot
$script:FailCount = 0
$script:PassCount = 0

function Assert([string]$RuleId, [string]$Title, [bool]$Ok, [string]$Detail) {
    if ($Ok) {
        $script:PassCount++
        Write-Host ("  [PASS] R{0}: {1}" -f $RuleId, $Title)
    } else {
        $script:FailCount++
        Write-Host ("  [FAIL] R{0}: {1}" -f $RuleId, $Title) -ForegroundColor Red
        Write-Host ("         " + $Detail) -ForegroundColor Yellow
    }
}

function Select-Grep([string[]]$Files, [string]$Pattern) {
    $hits = @()
    foreach ($f in $Files) {
        if (-not (Test-Path $f)) { continue }
        foreach ($line in (Get-Content $f -Encoding utf8)) {
            if ($line -match $Pattern) { $hits += $line.Trim() }
        }
    }
    return ,$hits   # preserve array identity even when empty
}

function Test-Rule1 {
    Write-Host ""
    Write-Host "=== R1: domain/Cargo.toml depends only on serde ==="
    $f = Join-Path $script:RepoRoot "crates\domain\Cargo.toml"
    $toml = Get-Content $f -Encoding utf8 -Raw
    $otherDeps = @([regex]::Matches($toml, '(?m)^\s*(\w[\w-]*)\s*=\s*\{') |
        ForEach-Object { $_.Groups[1].Value } | Where-Object { $_ -ne 'serde' })
    Assert 1 "domain depends only on serde" ($otherDeps.Count -eq 0) ("extra deps: " + ($otherDeps -join ', '))
}

function Test-Rule2 {
    Write-Host ""
    Write-Host "=== R2: application/src has no pool()/sqlx:: ==="
    $files = Get-ChildItem (Join-Path $script:RepoRoot "crates\application\src") -Recurse -Filter *.rs |
        ForEach-Object { $_.FullName }
    $hits = Select-Grep $files '(?:pool\(\)|sqlx::)'
    Assert 2 "application must not touch persistence directly" ($hits.Count -eq 0) ("{0} hits, first: {1}" -f $hits.Count, ($hits | Select-Object -First 1))
}

function Test-Rule3 {
    Write-Host ""
    Write-Host "=== R3: domain/src has no *Req struct ==="
    $files = Get-ChildItem (Join-Path $script:RepoRoot "crates\domain\src") -Recurse -Filter *.rs |
        ForEach-Object { $_.FullName }
    $hits = @()
    foreach ($f in $files) {
        foreach ($line in (Get-Content $f -Encoding utf8)) {
            if ($line -match 'pub\s+struct\s+\w+Req\s*\{') { $hits += ((Split-Path $f -Leaf) + ": " + $line.Trim()) }
        }
    }
    Assert 3 "domain must not own HTTP request contracts" ($hits.Count -eq 0) ("{0} hits, first: {1}" -f $hits.Count, ($hits | Select-Object -First 1))
}

function Test-Rule4 {
    Write-Host ""
    Write-Host "=== R4: backend/src (except di.rs) has no pool() ==="
    $files = Get-ChildItem (Join-Path $script:RepoRoot "crates\backend\src") -Recurse -Filter *.rs |
        Where-Object { $_.Name -ne 'di.rs' } | ForEach-Object { $_.FullName }
    $hits = Select-Grep $files 'pool\(\)'
    Assert 4 "only di.rs may call pool()" ($hits.Count -eq 0) ("{0} hits, first: {1}" -f $hits.Count, ($hits | Select-Object -First 1))
}

function Test-Rule5 {
    Write-Host ""
    Write-Host "=== R5: application/Cargo.toml has no database/sqlx deps ==="
    $f = Join-Path $script:RepoRoot "crates\application\Cargo.toml"
    $toml = Get-Content $f -Encoding utf8 -Raw
    $bad = $false
    if ($toml -match '^\s*database\s*=' -or $toml -match '^\s*sqlx\s*=') { $bad = $true }
    Assert 5 "application Cargo.toml free of database/sqlx" (-not $bad) "found database or sqlx dep"
}

$all = @('Test-Rule1','Test-Rule2','Test-Rule3','Test-Rule4','Test-Rule5')

if ($Rule -eq 0) {
    Write-Host "==============================================="
    Write-Host " HEXAGONAL LAYER CHECK (all 5 rules)"
    Write-Host " repo: $script:RepoRoot"
    Write-Host "==============================================="
    foreach ($fn in $all) { & $fn }
} else {
    Write-Host "Hexagonal Layer Check - R$Rule only"
    & $all[$Rule - 1]
}

Write-Host ""
Write-Host ("PASS={0}  FAIL={1}" -f $script:PassCount, $script:FailCount)
Write-Host ""

if ($script:FailCount -gt 0) { exit 1 }
exit 0

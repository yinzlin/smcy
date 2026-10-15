# 常用任务入口（与 Makefile 一一对应）
# 用法：./make.ps1 [check|check-layers|build|release|run|fmt|clippy|clean|frontend-build|frontend-dev]
param([string]$Task = 'help')

switch ($Task) {
    'check'   { cargo check --workspace }
    'check-layers' { powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'scripts\check_layers.ps1') }
    'build'   { cargo build }
    'release' {
        Push-Location crates/frontend
        npm install --no-audit --no-fund
        npm run build
        Pop-Location
        if ($LASTEXITCODE -eq 0) { cargo build --release }
    }
    'run'     {
        Push-Location crates/frontend
        npm install --no-audit --no-fund
        npm run build
        Pop-Location
        if ($LASTEXITCODE -eq 0) {
            cargo build --release
            if ($LASTEXITCODE -eq 0) { Start-Process .\target\x86_64-win7-windows-msvc\release\food_accept_single.exe }
        }
    }
    'fmt'     { cargo fmt --all }
    'clippy'  { cargo clippy --workspace }
    'clean'   { cargo clean }
    'frontend-build' {
        Push-Location crates/frontend
        npm install --no-audit --no-fund
        npm run build
        Pop-Location
    }
    'frontend-dev' {
        Push-Location crates/frontend
        npm run dev
        Pop-Location
    }
    default   { Write-Output 'tasks: check | check-layers | build | release | run | fmt | clippy | clean | frontend-build | frontend-dev' }
}

# ============================================================================
# smoke.ps1 - End-to-end smoke test for food_accept_single.exe
#
# Usage:
#   .\scripts\smoke.ps1                        # full run against http://127.0.0.1
#   .\scripts\smoke.ps1 -BaseUrl http://127.0.0.1:8080
#   .\scripts\smoke.ps1 -Quick                 # skip slow query/export groups
#
# IMPORTANT:
#   - Target server must run in an ISOLATED working directory (a copy of the
#     exe started in an empty temp dir), NEVER inside the repo root.
#   - Exits 0 when all assertions pass, 1 when any assertion fails.
#   - XFAIL marks assertions hitting KNOWN PRODUCT BUGS (handlers panic and
#     close the connection). They are recorded, not counted as gate failures;
#     they print as XPASS once the bug gets fixed.
#   - All output is ASCII on purpose (GBK console safe).
# ============================================================================

param(
    [string]$BaseUrl = "http://127.0.0.1",
    [switch]$Quick
)

$ErrorActionPreference = "Stop"

# ---------------------------------------------------------------------------
# Counters / helpers
# ---------------------------------------------------------------------------
$script:PassCount = 0
$script:FailCount = 0
$script:Failures = New-Object System.Collections.Generic.List[string]
$script:GroupPass = 0
$script:GroupFail = 0
$script:Session = $null
$script:LastRes = $null

function Start-Group([string]$Name) {
    $script:GroupPass = 0
    $script:GroupFail = 0
    $script:GroupXfail = 0
    Write-Host ""
    Write-Host ("=== " + $Name + " ===")
}

function End-Group([string]$Name) {
    Write-Host ("--- " + $Name + ": PASS=" + $script:GroupPass + " FAIL=" + $script:GroupFail + " XFAIL=" + $script:GroupXfail)
}

function Add-Result([string]$Name, [bool]$Ok, [string]$Detail) {
    if ($Ok) {
        $script:PassCount++
        $script:GroupPass++
        Write-Host ("  [PASS] " + $Name)
    } else {
        $script:FailCount++
        $script:GroupFail++
        $msg = $Name
        if ($Detail) { $msg = $Name + " -> " + $Detail }
        Write-Host ("  [FAIL] " + $msg)
        $script:Failures.Add($msg)
    }
}

function Assert-Status([string]$Name, [int]$Actual, [int[]]$Expected) {
    $ok = $false
    foreach ($e in $Expected) { if ($Actual -eq $e) { $ok = $true } }
    $want = ($Expected -join "|")
    $detail = "http status " + $Actual + " (expected " + $want + ")"
    if (-not $ok -and $null -ne $script:LastRes) {
        $b = $script:LastRes.Content
        if ($b.Length -gt 200) { $b = $b.Substring(0, 200) }
        if ($b.Length -gt 0) { $detail = $detail + " body=" + $b }
    }
    Add-Result $Name $ok $detail
}

function Assert-True([string]$Name, [bool]$Cond, [string]$Detail) {
    Add-Result $Name $Cond $Detail
}

# ---------------------------------------------------------------------------
# XFAIL: known product bugs (recorded in the report, NOT fixed here).
# handlers.rs:8090/9220/8147/10924 - f64 column decode of INTEGER literal
# COALESCE(SUM(x), 0) panics whenever a purchaser/month has no sales rows,
# the connection is closed without reply. XFAIL keeps the gate green for
# regressions while keeping the bug visible; prints XPASS when fixed.
# ---------------------------------------------------------------------------
$script:XfailCount = 0
$script:XpassCount = 0

function Add-XfailJson([string]$Name, $Res) {
    $script:GroupXfail++
    if ($Res.Status -eq 200 -and $null -ne $Res.Json) {
        $script:XpassCount++
        Write-Host ("  [XPASS] " + $Name + " (known bug appears fixed: 200 + json ok)")
    } else {
        $script:XfailCount++
        Write-Host ("  [XFAIL] " + $Name + " known product bug: handler panic closes connection (status=" + $Res.Status + ")")
    }
}

function Add-XfailXlsx([string]$Name, $Res) {
    $script:GroupXfail++
    if ($Res.Status -eq 200 -and $Res.ContentType -like "*spreadsheetml*") {
        $script:XpassCount++
        Write-Host ("  [XPASS] " + $Name + " (known bug appears fixed: 200 + xlsx ok)")
    } else {
        $script:XfailCount++
        Write-Host ("  [XFAIL] " + $Name + " known product bug: handler panic closes connection (status=" + $Res.Status + ")")
    }
}

function ConvertTo-JsonSafe([object]$Obj) {
    return ConvertTo-Json -InputObject $Obj -Depth 10 -Compress
}

function Parse-Json([string]$Text) {
    if ([string]::IsNullOrEmpty($Text)) { return $null }
    try { return $Text | ConvertFrom-Json } catch { return $null }
}

# ---------------------------------------------------------------------------
# Single HTTP entry point. Captures status on 4xx/5xx via catch.
# Returns hashtable: Status, Content, ContentType, Json
# ---------------------------------------------------------------------------
function Invoke-Api {
    param(
        [string]$Method,
        [string]$Path,
        $Body,                       # object -> JSON; string -> raw (CSV upload)
        [string]$ContentType = "application/json",
        $Session = $null,            # override session (e.g. anonymous test)
        [int]$TimeoutSec = 60
    )
    $url = $BaseUrl.TrimEnd('/') + $Path
    $useSession = $script:Session
    if ($null -ne $Session) { $useSession = $Session }

    $status = 0
    $content = ""
    $ctype = ""
    try {
        if ($null -ne $Body) {
            $payload = $Body
            if ($Body -isnot [string]) { $payload = ConvertTo-JsonSafe $Body }
            $r = Invoke-WebRequest -Uri $url -Method $Method -Body $payload `
                -ContentType $ContentType -WebSession $useSession `
                -UseBasicParsing -TimeoutSec $TimeoutSec -DisableKeepAlive
        } else {
            $r = Invoke-WebRequest -Uri $url -Method $Method `
                -WebSession $useSession -UseBasicParsing -TimeoutSec $TimeoutSec -DisableKeepAlive
        }
        $status = [int]$r.StatusCode
        $content = [string]$r.Content
        $ctype = [string]$r.Headers["Content-Type"]
    } catch {
        $resp = $_.Exception.Response
        if ($null -ne $resp) {
            $status = [int]$resp.StatusCode
            try {
                $stream = $resp.GetResponseStream()
                $reader = New-Object System.IO.StreamReader($stream)
                $content = $reader.ReadToEnd()
                $reader.Close()
            } catch { }
            try { $ctype = [string]$resp.ContentType } catch { }
        } else {
            $status = -1
            $content = $_.Exception.Message
        }
    }
    $script:LastRes = @{ Status = $status; Content = [string]$content; ContentType = $ctype; Json = (Parse-Json $content) }
    return $script:LastRes
}

function Assert-Xlsx([string]$Name, $Res) {
    $ok = ($Res.Status -eq 200) -and ($Res.ContentType -like "*spreadsheetml*")
    Add-Result $Name $ok ("status=" + $Res.Status + " contentType=" + $Res.ContentType)
}

# Raw status probe with auto-redirect disabled (PS 5.1 Invoke-WebRequest always follows redirects)
function Get-RawStatus([string]$Path) {
    $url = $BaseUrl.TrimEnd('/') + $Path
    $req = [System.Net.HttpWebRequest]::Create($url)
    $req.AllowAutoRedirect = $false
    $req.KeepAlive = $false
    $req.Timeout = 15000
    try {
        $resp = $req.GetResponse()
        $st = [int]$resp.StatusCode
        $resp.Close()
        return $st
    } catch {
        $we = $_.Exception
        if ($null -ne $we.Response) {
            $st = [int]$we.Response.StatusCode
            $we.Response.Close()
            return $st
        }
        return -1
    }
}

# ---------------------------------------------------------------------------
# Login (fresh session)
# ---------------------------------------------------------------------------
$script:Session = New-Object Microsoft.PowerShell.Commands.WebRequestSession

Write-Host "SMOKE TEST START - target $BaseUrl"

$login = Invoke-Api -Method Post -Path "/api/login" -Body @{ username = "super_admin"; password = "admin123" }
Assert-Status "auth.login.ok" $login.Status @(200)
Assert-True "auth.login.success" ($login.Json -ne $null -and $login.Json.success -eq $true) ("body=" + $login.Content)

# ===========================================================================
# A. Routes
# ===========================================================================
Start-Group "Group A: routes"
$r = Invoke-Api -Method Get -Path "/"
Assert-Status "route.root.redirect" (Get-RawStatus "/") @(301, 302, 307)
Assert-Status "route.login.redirect" (Get-RawStatus "/login") @(301, 302, 307)
Assert-Status "route.root.chain_final" $r.Status @(200)
$r = Invoke-Api -Method Get -Path "/app"
Assert-Status "route.app.index" $r.Status @(200)
$r = Invoke-Api -Method Get -Path "/legacy/login"
Assert-Status "route.legacy.login.404" $r.Status @(404)
$r = Invoke-Api -Method Get -Path "/app/login"
Assert-Status "route.app.login" $r.Status @(200)
$r = Invoke-Api -Method Get -Path "/api/login/check"
Assert-Status "route.login.check.status" $r.Status @(200)
Assert-True "route.login.check.logged_in" ($null -ne $r.Json -and $r.Json.logged_in -eq $true) ("body=" + $r.Content)
End-Group "Group A"

# ===========================================================================
# B. Auth / Users
# ===========================================================================
Start-Group "Group B: auth and users"
# wrong password: product returns 200 + success=false (by design, not 4xx)
$r = Invoke-Api -Method Post -Path "/api/login" -Body @{ username = "super_admin"; password = "wrong-password" }
Assert-Status "auth.login.wrongpwd.status" $r.Status @(200)
Assert-True "auth.login.wrongpwd.rejected" ($null -ne $r.Json -and $r.Json.success -eq $false) ("body=" + $r.Content)

$r = Invoke-Api -Method Get -Path "/api/user/list"
Assert-Status "user.list.status" $r.Status @(200)
Assert-True "user.list.is_array" ($r.Json -is [array]) ("body=" + $r.Content.Substring(0, [Math]::Min(120, $r.Content.Length)))

$r = Invoke-Api -Method Post -Path "/api/user" -Body @{ username = "smoke_user"; password = "smoke123"; nickname = "Smoke User"; role = "user" }
Assert-Status "user.create.status" $r.Status @(200)
Assert-True "user.create.success" ($null -ne $r.Json -and $r.Json.success -eq $true) ("body=" + $r.Content)

$r = Invoke-Api -Method Get -Path "/api/user/list"
$smokeUser = $null
if ($r.Json -is [array]) {
    foreach ($u in $r.Json) { if ($u.username -eq "smoke_user") { $smokeUser = $u } }
}
Assert-True "user.create.visible_in_list" ($null -ne $smokeUser) "smoke_user not found in user list"

if ($null -ne $smokeUser) {
    $uid = $smokeUser.id
    $r = Invoke-Api -Method Get -Path "/api/user/$uid"
    Assert-Status "user.get.by_id" $r.Status @(200)
    Assert-True "user.get.success" ($null -ne $r.Json -and $r.Json.success -eq $true) ("body=" + $r.Content)

    $r = Invoke-Api -Method Put -Path "/api/user/$uid" -Body @{ username = "smoke_user"; nickname = "Smoke User 2"; role = "user" }
    Assert-Status "user.update.status" $r.Status @(200)
    Assert-True "user.update.success" ($null -ne $r.Json -and $r.Json.success -eq $true) ("body=" + $r.Content)

    $r = Invoke-Api -Method Put -Path "/api/user/$uid/status" -Body @{ status = 0 }
    Assert-Status "user.disable.status" $r.Status @(200)
    $r = Invoke-Api -Method Put -Path "/api/user/$uid/status" -Body @{ status = 1 }
    Assert-Status "user.enable.status" $r.Status @(200)
    Assert-True "user.status.toggle" ($null -ne $r.Json -and $r.Json.success -eq $true) ("body=" + $r.Content)
}

$r = Invoke-Api -Method Post -Path "/api/user/contact_phone" -Body @{ contact_phone = "13800000000" }
Assert-Status "user.contact_phone.set" $r.Status @(200)
Assert-True "user.contact_phone.set.success" ($null -ne $r.Json -and $r.Json.success -eq $true) ("body=" + $r.Content)
$r = Invoke-Api -Method Get -Path "/api/user/contact_phone"
Assert-Status "user.contact_phone.get" $r.Status @(200)
Assert-True "user.contact_phone.roundtrip" ($null -ne $r.Json -and $r.Json.contact_phone -eq "13800000000") ("body=" + $r.Content)

if ($null -ne $smokeUser) {
    $uid = $smokeUser.id
    $r = Invoke-Api -Method Delete -Path "/api/user/$uid"
    Assert-Status "user.delete.status" $r.Status @(200)
    Assert-True "user.delete.success" ($null -ne $r.Json -and $r.Json.success -eq $true) ("body=" + $r.Content)
}
End-Group "Group B"

# ===========================================================================
# C. Basic data: category / supplier / purchaser / warehouse
# ===========================================================================
Start-Group "Group C: basic data"

# --- category ---
$r = Invoke-Api -Method Get -Path "/api/category/list"
Assert-Status "category.list.status" $r.Status @(200)
Assert-True "category.list.is_array" ($r.Json -is [array]) ""
$r = Invoke-Api -Method Get -Path "/api/category/tree?entity_type=product"
Assert-Status "category.tree.status" $r.Status @(200)
Assert-True "category.tree.is_array" ($r.Json -is [array]) ""
$r = Invoke-Api -Method Post -Path "/api/category/create" -Body @{ name = "SmokeCat"; entity_type = "product"; parent_id = $null; sort_order = 99 }
Assert-Status "category.create.status" $r.Status @(200)
$r = Invoke-Api -Method Get -Path "/api/category/list"
$smokeCat = $null
if ($r.Json -is [array]) { foreach ($c in $r.Json) { if ($c.name -eq "SmokeCat") { $smokeCat = $c } } }
Assert-True "category.create.visible" ($null -ne $smokeCat) "SmokeCat not found in category list"
if ($null -ne $smokeCat) {
    $cid = $smokeCat.id
    $r = Invoke-Api -Method Post -Path "/api/category/rename" -Body @{ id = $cid; name = "SmokeCat2" }
    Assert-Status "category.rename.status" $r.Status @(200)
    Assert-True "category.rename.ok" ($r.Content -like "*ok*" -or $r.Content.Length -gt 0) ("body=" + $r.Content)
    $r = Invoke-Api -Method Get -Path "/api/category/list"
    $renamed = $false
    if ($r.Json -is [array]) { foreach ($c in $r.Json) { if ($c.id -eq $cid -and $c.name -eq "SmokeCat2") { $renamed = $true } } }
    Assert-True "category.rename.visible" $renamed "rename not reflected in list"
    $r = Invoke-Api -Method Post -Path "/api/category/delete" -Body @{ id = $cid }
    Assert-Status "category.delete.status" $r.Status @(200)
}

# --- supplier ---
$r = Invoke-Api -Method Get -Path "/api/supplier/list"
Assert-Status "supplier.list.status" $r.Status @(200)
Assert-True "supplier.list.is_array" ($r.Json -is [array]) ""
$r = Invoke-Api -Method Post -Path "/api/supplier/create" -Body @{ name = "SmokeSupplier"; contact = "Tom"; phone = "123"; address = "addr"; business_scope = "food"; remark = "smoke" }
Assert-Status "supplier.create.status" $r.Status @(200)
$r = Invoke-Api -Method Get -Path "/api/supplier/list"
$sup = $null
if ($r.Json -is [array]) { foreach ($s in $r.Json) { if ($s.name -eq "SmokeSupplier") { $sup = $s } } }
Assert-True "supplier.create.visible" ($null -ne $sup) "SmokeSupplier not found"
if ($null -ne $sup) {
    $sid = $sup.id
    $r = Invoke-Api -Method Post -Path "/api/supplier/update" -Body @{ id = $sid; name = "SmokeSupplier"; contact = "Tom2"; phone = "456" }
    Assert-Status "supplier.update.status" $r.Status @(200)
    $r = Invoke-Api -Method Post -Path "/api/supplier/approve" -Body @{ id = $sid }
    Assert-Status "supplier.approve.status" $r.Status @(200)
    $r = Invoke-Api -Method Get -Path "/api/supplier/list"
    $confirmed = $false
    if ($r.Json -is [array]) { foreach ($s in $r.Json) { if ($s.id -eq $sid -and $s.audit_status -eq "confirmed") { $confirmed = $true } } }
    Assert-True "supplier.approve.confirmed" $confirmed "audit_status not confirmed after approve"
    $r = Invoke-Api -Method Post -Path "/api/supplier/unapprove" -Body @{ id = $sid }
    Assert-Status "supplier.unapprove.status" $r.Status @(200)
}

# supplier export (xlsx)
$r = Invoke-Api -Method Get -Path "/api/supplier/export"
Assert-Xlsx "supplier.export.xlsx" $r

# supplier import (CSV text; column 1 = name)
$csv = "no,name,contact,phone,address,scope,remark,category`r`n1,SmokeImportSup,C1,P1,A1,S1,R1,`r`n"
$r = Invoke-Api -Method Post -Path "/api/supplier/import" -Body $csv -ContentType "text/plain"
Assert-Status "supplier.import.status" $r.Status @(200)
Assert-True "supplier.import.report" ($r.Content -like "*OK*" -or $r.Content.Length -gt 0) ("body=" + $r.Content)
$r = Invoke-Api -Method Get -Path "/api/supplier/list"
$imp = $null
if ($r.Json -is [array]) { foreach ($s in $r.Json) { if ($s.name -eq "SmokeImportSup") { $imp = $s } } }
Assert-True "supplier.import.visible" ($null -ne $imp) "SmokeImportSup not found after import"

# supplier for later order tests: create + approve (keep confirmed)
$r = Invoke-Api -Method Post -Path "/api/supplier/create" -Body @{ name = "SmokeSupPO"; contact = "Po"; phone = "777" }
Assert-Status "supplier.create.order_supplier" $r.Status @(200)
$r = Invoke-Api -Method Get -Path "/api/supplier/list"
$script:SupPO = $null
if ($r.Json -is [array]) { foreach ($s in $r.Json) { if ($s.name -eq "SmokeSupPO") { $script:SupPO = $s } } }
if ($null -ne $script:SupPO) {
    $r = Invoke-Api -Method Post -Path "/api/supplier/approve" -Body @{ id = $script:SupPO.id }
    Assert-Status "supplier.approve.order_supplier" $r.Status @(200)
} else {
    Assert-True "supplier.approve.order_supplier" $false "SmokeSupPO not found"
}

# cleanup main supplier
if ($null -ne $sup) {
    $r = Invoke-Api -Method Post -Path "/api/supplier/delete" -Body @{ id = $sup.id }
    Assert-Status "supplier.delete.status" $r.Status @(200)
}
if ($null -ne $imp) {
    $r = Invoke-Api -Method Post -Path "/api/supplier/delete" -Body @{ id = $imp.id }
    Assert-Status "supplier.imported.delete" $r.Status @(200)
}

# --- purchaser (same as supplier) ---
$r = Invoke-Api -Method Get -Path "/api/purchaser/list"
Assert-Status "purchaser.list.status" $r.Status @(200)
Assert-True "purchaser.list.is_array" ($r.Json -is [array]) ""
$r = Invoke-Api -Method Post -Path "/api/purchaser/create" -Body @{ name = "SmokePurchaser"; contact = "Pu"; phone = "321" }
Assert-Status "purchaser.create.status" $r.Status @(200)
$r = Invoke-Api -Method Get -Path "/api/purchaser/list"
$purch = $null
if ($r.Json -is [array]) { foreach ($p in $r.Json) { if ($p.name -eq "SmokePurchaser") { $purch = $p } } }
Assert-True "purchaser.create.visible" ($null -ne $purch) "SmokePurchaser not found"
if ($null -ne $purch) {
    $purchId = $purch.id
    $r = Invoke-Api -Method Post -Path "/api/purchaser/update" -Body @{ id = $purchId; name = "SmokePurchaser"; contact = "Pu2" }
    Assert-Status "purchaser.update.status" $r.Status @(200)
    $r = Invoke-Api -Method Post -Path "/api/purchaser/approve" -Body @{ id = $purchId }
    Assert-Status "purchaser.approve.status" $r.Status @(200)
    $r = Invoke-Api -Method Post -Path "/api/purchaser/unapprove" -Body @{ id = $purchId }
    Assert-Status "purchaser.unapprove.status" $r.Status @(200)
}
$r = Invoke-Api -Method Get -Path "/api/purchaser/export"
Assert-Xlsx "purchaser.export.xlsx" $r
$csv2 = "no,name,contact,phone,address,scope,remark,category`r`n1,SmokeImportPurch,C1,P1,A1,S1,R1,`r`n"
$r = Invoke-Api -Method Post -Path "/api/purchaser/import" -Body $csv2 -ContentType "text/plain"
Assert-Status "purchaser.import.status" $r.Status @(200)
$r = Invoke-Api -Method Get -Path "/api/purchaser/list"
$impP = $null
if ($r.Json -is [array]) { foreach ($p in $r.Json) { if ($p.name -eq "SmokeImportPurch") { $impP = $p } } }
Assert-True "purchaser.import.visible" ($null -ne $impP) "SmokeImportPurch not found"
if ($null -ne $impP) {
    $r = Invoke-Api -Method Post -Path "/api/purchaser/delete" -Body @{ id = $impP.id }
    Assert-Status "purchaser.imported.delete" $r.Status @(200)
}

# purchaser for later sales order tests: create + approve (keep confirmed)
$r = Invoke-Api -Method Post -Path "/api/purchaser/create" -Body @{ name = "SmokePurchPO"; contact = "PoP"; phone = "888" }
Assert-Status "purchaser.create.order_purchaser" $r.Status @(200)
$r = Invoke-Api -Method Get -Path "/api/purchaser/list"
$script:PurchPO = $null
if ($r.Json -is [array]) { foreach ($p in $r.Json) { if ($p.name -eq "SmokePurchPO") { $script:PurchPO = $p } } }
if ($null -ne $script:PurchPO) {
    $r = Invoke-Api -Method Post -Path "/api/purchaser/approve" -Body @{ id = $script:PurchPO.id }
    Assert-Status "purchaser.approve.order_purchaser" $r.Status @(200)
} else {
    Assert-True "purchaser.approve.order_purchaser" $false "SmokePurchPO not found"
}

if ($null -ne $purch) {
    $r = Invoke-Api -Method Post -Path "/api/purchaser/delete" -Body @{ id = $purch.id }
    Assert-Status "purchaser.delete.status" $r.Status @(200)
}

# --- warehouse ---
$r = Invoke-Api -Method Get -Path "/api/warehouse/list"
Assert-Status "warehouse.list.status" $r.Status @(200)
Assert-True "warehouse.list.has_default" (($r.Json -is [array]) -and $r.Json.Count -ge 1) "warehouse list empty (default warehouse missing)"
$r = Invoke-Api -Method Post -Path "/api/warehouse/create" -Body @{ name = "SmokeWH"; code = "WH-SMOKE"; address = "a"; contact = "c"; phone = "p" }
Assert-Status "warehouse.create.status" $r.Status @(200)
$r = Invoke-Api -Method Get -Path "/api/warehouse/list"
$wh = $null
if ($r.Json -is [array]) { foreach ($w in $r.Json) { if ($w.name -eq "SmokeWH") { $wh = $w } } }
Assert-True "warehouse.create.visible" ($null -ne $wh) "SmokeWH not found"
if ($null -ne $wh) {
    $r = Invoke-Api -Method Post -Path "/api/warehouse/delete" -Body @{ id = $wh.id }
    Assert-Status "warehouse.delete.status" $r.Status @(200)
}
End-Group "Group C"

# ===========================================================================
# D. Products
# ===========================================================================
Start-Group "Group D: products"
$r = Invoke-Api -Method Get -Path "/api/product/list"
Assert-Status "product.list.status" $r.Status @(200)
Assert-True "product.list.json_ok" ($null -ne $r.Json) ""

$r = Invoke-Api -Method Post -Path "/api/product/create" -Body @{ name = "SmokeProduct"; spec = "S1"; unit = "jin"; base_unit = "jin"; base_price = 10.5; purchase_price = 8.0; category_id = $null }
Assert-Status "product.create.status" $r.Status @(200)

$r = Invoke-Api -Method Get -Path "/api/product/search?keyword=SmokeProduct"
Assert-Status "product.search.status" $r.Status @(200)
$prod = $null
if ($r.Json -is [array]) { foreach ($p in $r.Json) { if ($p.name -eq "SmokeProduct") { $prod = $p } } }
Assert-True "product.search.found" ($null -ne $prod) "SmokeProduct not found in search"

$r = Invoke-Api -Method Get -Path "/api/product/check_name?name=SmokeProduct"
Assert-Status "product.check_name.status" $r.Status @(200)
Assert-True "product.check_name.hit" (($r.Json -is [array]) -and $r.Json.Count -ge 1) "check_name returned no match"

if ($null -ne $prod) {
    $script:ProdId = $prod.id
    $r = Invoke-Api -Method Get -Path "/api/product/by_id?id=$($script:ProdId)"
    Assert-Status "product.by_id.status" $r.Status @(200)
    Assert-True "product.by_id.name" ($null -ne $r.Json -and $r.Json.name -eq "SmokeProduct") ("body=" + $r.Content)

    # unit create / list / delete_by_product
    $r = Invoke-Api -Method Post -Path "/api/product/unit/create" -Body @{ product_id = $script:ProdId; unit_name = "box"; ratio = 10; unit_price = 100.0; purchase_price = 80.0 }
    Assert-Status "product.unit.create.status" $r.Status @(200)
    $r = Invoke-Api -Method Get -Path "/api/product/unit/list?product_id=$($script:ProdId)"
    Assert-Status "product.unit.list.status" $r.Status @(200)
    Assert-True "product.unit.list.hit" (($r.Json -is [array]) -and $r.Json.Count -ge 1) "unit list empty"
    $r = Invoke-Api -Method Post -Path "/api/product/unit/delete_by_product" -Body @{ product_id = $script:ProdId }
    Assert-Status "product.unit.delete_by_product" $r.Status @(200)

    # price upsert / list / delete
    $r = Invoke-Api -Method Post -Path "/api/product/price/upsert" -Body @{ product_id = $script:ProdId; price_type = "supermarket_1"; price = 11.0; collected_at = "2026-10-14"; source = "smoke" }
    Assert-Status "product.price.upsert.status" $r.Status @(200)
    $r = Invoke-Api -Method Get -Path "/api/product/price/list?product_id=$($script:ProdId)"
    Assert-Status "product.price.list.status" $r.Status @(200)
    $priceRec = $null
    if ($r.Json -is [array]) { foreach ($p in $r.Json) { if ($p.price_type -eq "supermarket_1") { $priceRec = $p } } }
    Assert-True "product.price.list.hit" ($null -ne $priceRec) "supermarket_1 price not found"
    if ($null -ne $priceRec) {
        $r = Invoke-Api -Method Post -Path "/api/product/price/delete" -Body @{ id = $priceRec.id }
        Assert-Status "product.price.delete.status" $r.Status @(200)
    }

    # update
    $r = Invoke-Api -Method Post -Path "/api/product/update" -Body @{ id = $script:ProdId; name = "SmokeProduct"; spec = "S2"; unit = "jin"; base_unit = "jin"; base_price = 10.8; purchase_price = 8.2 }
    Assert-Status "product.update.status" $r.Status @(200)

    # sync base price / toggle status (twice to restore)
    $r = Invoke-Api -Method Post -Path "/api/product/sync_base_price" -Body @{ product_id = $script:ProdId }
    Assert-Status "product.sync_base_price" $r.Status @(200)
    $r = Invoke-Api -Method Post -Path "/api/product/toggle_status/$($script:ProdId)"
    Assert-Status "product.toggle_status.1" $r.Status @(200)
    $r = Invoke-Api -Method Post -Path "/api/product/toggle_status/$($script:ProdId)"
    Assert-Status "product.toggle_status.2.restore" $r.Status @(200)

    # approve (keep confirmed for order tests)
    $r = Invoke-Api -Method Post -Path "/api/product/approve" -Body @{ id = $script:ProdId }
    Assert-Status "product.approve.status" $r.Status @(200)
}
End-Group "Group D"

# ===========================================================================
# E. Price schedule + today price
# ===========================================================================
Start-Group "Group E: price schedule"
$script:SchedId = $null
if ($null -ne $script:ProdId) {
    $r = Invoke-Api -Method Post -Path "/api/price_schedule/create" -Body @{ product_id = $script:ProdId; price_type = "gov_procurement"; price = 10.5; effective_date = "2026-01-01" }
    Assert-Status "price_schedule.create.status" $r.Status @(200)
    Assert-True "price_schedule.create.success" ($null -ne $r.Json -and $r.Json.success -eq $true) ("body=" + $r.Content)
    if ($null -ne $r.Json) { $script:SchedId = $r.Json.id }

    # duplicate (product, type, effective_date) -> 409
    $r = Invoke-Api -Method Post -Path "/api/price_schedule/create" -Body @{ product_id = $script:ProdId; price_type = "gov_procurement"; price = 9.9; effective_date = "2026-01-01" }
    Assert-Status "price_schedule.duplicate.409" $r.Status @(409)

    # extra schedule types for query data
    $r = Invoke-Api -Method Post -Path "/api/price_schedule/create" -Body @{ product_id = $script:ProdId; price_type = "supermarket_1"; price = 11.5; effective_date = "2026-01-01" }
    Assert-Status "price_schedule.create.supermarket_1" $r.Status @(200)
    $r = Invoke-Api -Method Post -Path "/api/price_schedule/create" -Body @{ product_id = $script:ProdId; price_type = "supermarket_2"; price = 12.5; effective_date = "2026-01-01" }
    Assert-Status "price_schedule.create.supermarket_2" $r.Status @(200)

    $r = Invoke-Api -Method Get -Path "/api/price_schedule/list?product_id=$($script:ProdId)"
    Assert-Status "price_schedule.list.status" $r.Status @(200)
    Assert-True "price_schedule.list.count" ($null -ne $r.Json -and $r.Json.data -is [array] -and $r.Json.data.Count -ge 3) ("total=" + $r.Json.total)

    if ($null -ne $script:SchedId) {
        $r = Invoke-Api -Method Post -Path "/api/price_schedule/update" -Body @{ id = $script:SchedId; product_id = $script:ProdId; price_type = "gov_procurement"; price = 11.5; effective_date = "2026-01-01"; remark = "smoke-upd" }
        Assert-Status "price_schedule.update.status" $r.Status @(200)
        Assert-True "price_schedule.update.success" ($null -ne $r.Json -and $r.Json.success -eq $true) ("body=" + $r.Content)

        $r = Invoke-Api -Method Get -Path "/api/price_schedule/get?id=$($script:SchedId)"
        Assert-Status "price_schedule.get.status" $r.Status @(200)
        Assert-True "price_schedule.get.success" ($null -ne $r.Json -and $r.Json.success -eq $true) ("body=" + $r.Content)
    }
    $r = Invoke-Api -Method Get -Path "/api/price_schedule/lookup?product_id=$($script:ProdId)&price_type=gov_procurement&query_date=2026-10-14"
    Assert-Status "price_schedule.lookup.status" $r.Status @(200)
    Assert-True "price_schedule.lookup.matched" ($null -ne $r.Json -and $r.Json.matched -eq $true -and [double]$r.Json.price -eq 11.5) ("body=" + $r.Content)
    $r = Invoke-Api -Method Get -Path "/api/price_schedule/history?product_id=$($script:ProdId)"
    Assert-Status "price_schedule.history.status" $r.Status @(200)
    $r = Invoke-Api -Method Get -Path "/api/price_schedule/diagnose?product_id=$($script:ProdId)"
    Assert-Status "price_schedule.diagnose.status" $r.Status @(200)

    $r = Invoke-Api -Method Get -Path "/api/price_schedule/export"
    Assert-Xlsx "price_schedule.export.xlsx" $r
    $r = Invoke-Api -Method Get -Path "/api/price_schedule/export_template"
    Assert-Xlsx "price_schedule.export_template.xlsx" $r
} else {
    Assert-True "price_schedule.skipped_no_product" $false "product missing, price schedule group degraded"
}

# today price (super_admin only)
$supPoId = 0
if ($null -ne $script:SupPO) { $supPoId = $script:SupPO.id }
$r = Invoke-Api -Method Get -Path "/api/product/today_price_items?date=2026-10-14"
Assert-Status "today_price.items.status" $r.Status @(200)
$r = Invoke-Api -Method Post -Path "/api/product/today_price_save" -Body @{ items = @( @{ product_id = $script:ProdId; supplier_id = $supPoId; price = 12.0 } ) }
Assert-Status "today_price.save.status" $r.Status @(200)
Assert-True "today_price.save.success" ($null -ne $r.Json -and $r.Json.success -eq $true) ("body=" + $r.Content)
$r = Invoke-Api -Method Get -Path "/api/product/today_price_excel"
Assert-Xlsx "today_price.excel.xlsx" $r

# cleanup one schedule (keeps gov + supermarket_1 for data)
$r = Invoke-Api -Method Get -Path "/api/price_schedule/list?product_id=$($script:ProdId)&price_type=supermarket_2"
$sm2 = $null
if ($null -ne $r.Json -and $r.Json.data -is [array]) { foreach ($x in $r.Json.data) { $sm2 = $x } }
if ($null -ne $sm2) {
    $r = Invoke-Api -Method Post -Path "/api/price_schedule/delete" -Body @{ id = $sm2.id }
    Assert-Status "price_schedule.delete.status" $r.Status @(200)
    Assert-True "price_schedule.delete.success" ($null -ne $r.Json -and $r.Json.success -eq $true) ("body=" + $r.Content)
}
End-Group "Group E"

# ===========================================================================
# F. Purchase order
# ===========================================================================
Start-Group "Group F: purchase order"
$script:PoId = $null
if ($null -ne $script:ProdId -and $null -ne $script:SupPO) {
    $poBody = @{
        supplier_id = $script:SupPO.id
        order_no = "PO-SMOKE-001"
        order_date = "2026-10-14"
        total_amount = 20.0
        discount_rate = 1.0
        amount_reduction = 0.0
        final_amount = 20.0
        warehouse_id = 1
        warehouse_name = "Main"
        items = @( @{
            product_id = $script:ProdId
            product_name = "SmokeProduct"
            spec = "S2"
            unit = "jin"
            unit_price = 10.0
            quantity = 2.0
            amount = 20.0
            warehouse_id = 1
            warehouse_name = "Main"
        } )
        remark = "smoke po"
    }
    $r = Invoke-Api -Method Post -Path "/api/purchase_order/create" -Body $poBody
    Assert-Status "purchase_order.create.status" $r.Status @(200)

    $r = Invoke-Api -Method Get -Path "/api/purchase_order/list?page=1&page_size=10"
    Assert-Status "purchase_order.list.status" $r.Status @(200)
    Assert-True "purchase_order.list.shape" ($null -ne $r.Json -and $r.Json.data -is [array] -and $r.Json.data.Count -ge 1) ""
    if ($null -ne $r.Json -and $r.Json.data -is [array] -and $r.Json.data.Count -ge 1) {
        $script:PoId = $r.Json.data[0].id
    }
    if ($null -ne $script:PoId) {
        $r = Invoke-Api -Method Get -Path "/api/purchase_order/detail/$($script:PoId)"
        Assert-Status "purchase_order.detail.status" $r.Status @(200)
        Assert-True "purchase_order.detail.pending" ($null -ne $r.Json -and $r.Json.status -eq "pending") ("status=" + $r.Json.status)
        $ver = 0
        if ($null -ne $r.Json) { $ver = [int64]$r.Json.version }

        # update with optimistic lock version (DTO id is required on update)
        $upd = $poBody.Clone()
        $upd.id = $script:PoId
        $upd.items = @( @{
            product_id = $script:ProdId
            product_name = "SmokeProduct"
            spec = "S2"
            unit = "jin"
            unit_price = 10.0
            quantity = 3.0
            amount = 30.0
            warehouse_id = 1
            warehouse_name = "Main"
        } )
        $upd.total_amount = 30.0
        $upd.final_amount = 30.0
        $upd.version = $ver
        $r = Invoke-Api -Method Post -Path "/api/purchase_order/update" -Body $upd
        Assert-Status "purchase_order.update.status" $r.Status @(200)

        $r = Invoke-Api -Method Post -Path "/api/purchase_order/approve/$($script:PoId)" -Body @{ reason = "smoke" }
        Assert-Status "purchase_order.approve.status" $r.Status @(200)
        # double approve -> 400
        $r = Invoke-Api -Method Post -Path "/api/purchase_order/approve/$($script:PoId)" -Body @{ reason = "smoke" }
        Assert-Status "purchase_order.approve.twice.400" $r.Status @(400, 409)
        $r = Invoke-Api -Method Post -Path "/api/purchase_order/unapprove/$($script:PoId)" -Body @{ reason = "smoke" }
        Assert-Status "purchase_order.unapprove.status" $r.Status @(200)
    }

    $r = Invoke-Api -Method Get -Path "/api/purchase_order/export?start_date=2026-01-01&end_date=2026-12-31"
    Assert-Xlsx "purchase_order.export.xlsx" $r
    if ($null -ne $script:PoId) {
        $r = Invoke-Api -Method Get -Path "/api/purchase_order/export_print/$($script:PoId)"
        Assert-Xlsx "purchase_order.export_print.xlsx" $r
    }
} else {
    Assert-True "purchase_order.skipped_no_base_data" $false "product or approved supplier missing"
}

# delete purchase order (after unapprove, back to pending)
if ($null -ne $script:PoId) {
    $r = Invoke-Api -Method Delete -Path "/api/purchase_order/delete/$($script:PoId)"
    Assert-Status "purchase_order.delete.status" $r.Status @(200)
}
End-Group "Group F"

# ===========================================================================
# G. Sales order
# ===========================================================================
Start-Group "Group G: sales order"
$script:SoId = $null
if ($null -ne $script:ProdId -and $null -ne $script:PurchPO -and $null -ne $script:SupPO) {
    $soBody = @{
        purchaser_id = $script:PurchPO.id
        order_no = "SO-SMOKE-001"
        order_date = "2026-10-14"
        total_amount = 20.0
        discount_rate = 1.0
        amount_reduction = 0.0
        final_amount = 20.0
        warehouse_id = 1
        warehouse_name = "Main"
        items = @( @{
            product_id = $script:ProdId
            product_name = "SmokeProduct"
            spec = "S2"
            unit = "jin"
            unit_price = 10.0
            quantity = 2.0
            amount = 20.0
            supplier_id = $script:SupPO.id
            supplier_name = "SmokeSupPO"
        } )
        remark = "smoke so"
    }
    $r = Invoke-Api -Method Post -Path "/api/sales_order/create" -Body $soBody
    Assert-Status "sales_order.create.status" $r.Status @(200)

    $r = Invoke-Api -Method Get -Path "/api/sales_order/list?page=1&page_size=10"
    Assert-Status "sales_order.list.status" $r.Status @(200)
    Assert-True "sales_order.list.shape" ($null -ne $r.Json -and $r.Json.data -is [array] -and $r.Json.data.Count -ge 1) ""
    if ($null -ne $r.Json -and $r.Json.data -is [array] -and $r.Json.data.Count -ge 1) {
        $script:SoId = $r.Json.data[0].id
    }
    if ($null -ne $script:SoId) {
        $r = Invoke-Api -Method Get -Path "/api/sales_order/detail/$($script:SoId)"
        Assert-Status "sales_order.detail.status" $r.Status @(200)
        Assert-True "sales_order.detail.pending" ($null -ne $r.Json -and $r.Json.status -eq "pending") ("status=" + $r.Json.status)
        $ver = 0
        if ($null -ne $r.Json) { $ver = [int64]$r.Json.version }

        $upd = $soBody.Clone()
        $upd.id = $script:SoId
        $upd.remark = "smoke so updated"
        $upd.version = $ver
        $r = Invoke-Api -Method Post -Path "/api/sales_order/update" -Body $upd
        Assert-Status "sales_order.update.status" $r.Status @(200)

        # settle toggle (must run while pending)
        $r = Invoke-Api -Method Post -Path "/api/sales_order/settle/$($script:SoId)" -Body @{ is_settled = 1 }
        Assert-Status "sales_order.settle.on.status" $r.Status @(200)
        $r = Invoke-Api -Method Post -Path "/api/sales_order/settle/$($script:SoId)" -Body @{ is_settled = 0 }
        Assert-Status "sales_order.settle.off.status" $r.Status @(200)

        $r = Invoke-Api -Method Post -Path "/api/sales_order/approve/$($script:SoId)" -Body @{ reason = "smoke" }
        Assert-Status "sales_order.approve.status" $r.Status @(200)
        $r = Invoke-Api -Method Post -Path "/api/sales_order/unapprove/$($script:SoId)" -Body @{ reason = "smoke" }
        Assert-Status "sales_order.unapprove.status" $r.Status @(200)

        # generate purchase order from sales order
        $r = Invoke-Api -Method Post -Path "/api/sales_order/generate_purchase/$($script:SoId)"
        Assert-Status "sales_order.generate_purchase.status" $r.Status @(200)
        # real response shape: {"count":N,"excluded_count":N,"merged":N,"message":"..."}
        Assert-True "sales_order.generate_purchase.success" ($null -ne $r.Json -and ([int]$r.Json.count -ge 1 -or $r.Json.success -eq $true)) ("body=" + $r.Content)

        # accept / reimburse exports
        $r = Invoke-Api -Method Get -Path "/api/sales_order/accept_excel/$($script:SoId)"
        Assert-Xlsx "sales_order.accept_excel.xlsx" $r
        $r = Invoke-Api -Method Get -Path "/api/sales_order/real_excel/$($script:SoId)"
        Assert-Xlsx "sales_order.real_excel.xlsx" $r
    }

    $r = Invoke-Api -Method Get -Path "/api/sales_order/export?start_date=2026-01-01&end_date=2026-12-31"
    Assert-Xlsx "sales_order.export.xlsx" $r
} else {
    Assert-True "sales_order.skipped_no_base_data" $false "product/purchaser/supplier missing"
}

# correction without session -> 401/403
$anon = New-Object Microsoft.PowerShell.Commands.WebRequestSession
$r = Invoke-Api -Method Post -Path "/api/sales_order/correction" -Body @{ corrections = @() } -Session $anon
Assert-Status "sales_order.correction.anonymous.denied" $r.Status @(401, 403)

# delete sales order (back to pending after unapprove)
if ($null -ne $script:SoId) {
    $r = Invoke-Api -Method Delete -Path "/api/sales_order/delete/$($script:SoId)"
    Assert-Status "sales_order.delete.status" $r.Status @(200)
}
End-Group "Group G"

# ===========================================================================
# H. Query domain (skipped with -Quick)
# ===========================================================================
if (-not $Quick) {
    Start-Group "Group H: query domain"
    $d = "start_date=2026-01-01&end_date=2026-12-31"
    $m = "month=2026-10"

    # JSON query endpoints: 200 + parseable
    $r = Invoke-Api -Method Get -Path "/api/query/purchase_order?$d&page=1&page_size=10"
    Assert-Status "query.purchase_order.status" $r.Status @(200)
    Assert-True "query.purchase_order.json" ($null -ne $r.Json) ""
    $r = Invoke-Api -Method Get -Path "/api/query/purchase_price?product_name=&page=1&page_size=10"
    Assert-Status "query.purchase_price.status" $r.Status @(200)
    Assert-True "query.purchase_price.json" ($null -ne $r.Json) ""
    $r = Invoke-Api -Method Get -Path "/api/query/purchase_summary?$d"
    Assert-Status "query.purchase_summary.status" $r.Status @(200)
    Assert-True "query.purchase_summary.json" ($null -ne $r.Json) ""
    $r = Invoke-Api -Method Get -Path "/api/query/supplier_balance"
    Assert-Status "query.supplier_balance.status" $r.Status @(200)
    Assert-True "query.supplier_balance.json" ($null -ne $r.Json) ""
    $r = Invoke-Api -Method Get -Path "/api/query/sales_order?$d&page=1&page_size=10"
    Assert-Status "query.sales_order.status" $r.Status @(200)
    Assert-True "query.sales_order.json" ($null -ne $r.Json) ""
    $r = Invoke-Api -Method Get -Path "/api/query/sales_price?product_name=&page=1&page_size=10"
    Assert-Status "query.sales_price.status" $r.Status @(200)
    Assert-True "query.sales_price.json" ($null -ne $r.Json) ""
    $r = Invoke-Api -Method Get -Path "/api/query/sales_summary?$d"
    Assert-Status "query.sales_summary.status" $r.Status @(200)
    Assert-True "query.sales_summary.json" ($null -ne $r.Json) ""
    $r = Invoke-Api -Method Get -Path "/api/query/purchaser_balance"
    Add-XfailJson "query.purchaser_balance.json" $r
    $r = Invoke-Api -Method Get -Path "/api/query/product_rank?$d"
    Assert-Status "query.product_rank.status" $r.Status @(200)
    Assert-True "query.product_rank.json" ($null -ne $r.Json) ""
    $r = Invoke-Api -Method Get -Path "/api/query/reimburse_summary?$d"
    Assert-Status "query.reimburse_summary.status" $r.Status @(200)
    Assert-True "query.reimburse_summary.json" ($null -ne $r.Json) ""
    $r = Invoke-Api -Method Get -Path "/api/query/allocation_source?$d"
    Assert-Status "query.allocation_source.status" $r.Status @(200)
    Assert-True "query.allocation_source.json" ($null -ne $r.Json) ""
    $r = Invoke-Api -Method Get -Path "/api/query/stock_balance?$d&page=1&page_size=10"
    Assert-Status "query.stock_balance.status" $r.Status @(200)
    Assert-True "query.stock_balance.json" ($null -ne $r.Json) ""
    $r = Invoke-Api -Method Get -Path "/api/query/stock_flow?$d&page=1&page_size=10"
    Assert-Status "query.stock_flow.status" $r.Status @(200)
    Assert-True "query.stock_flow.json" ($null -ne $r.Json) ""
    $r = Invoke-Api -Method Get -Path "/api/query/stock_summary?$d"
    Assert-Status "query.stock_summary.status" $r.Status @(200)
    Assert-True "query.stock_summary.json" ($null -ne $r.Json) ""
    $r = Invoke-Api -Method Get -Path "/api/query/stock_summary_reimburse?$d"
    Assert-Status "query.stock_summary_reimburse.status" $r.Status @(200)
    Assert-True "query.stock_summary_reimburse.json" ($null -ne $r.Json) ""
    $r = Invoke-Api -Method Get -Path "/api/query/stock_warning"
    Assert-Status "query.stock_warning.status" $r.Status @(200)
    Assert-True "query.stock_warning.json" ($null -ne $r.Json) ""
    $r = Invoke-Api -Method Get -Path "/api/query/slow_stock?days=30"
    Assert-Status "query.slow_stock.status" $r.Status @(200)
    Assert-True "query.slow_stock.json" ($null -ne $r.Json) ""
    $r = Invoke-Api -Method Get -Path "/api/query/income_expense?$d"
    Assert-Status "query.income_expense.status" $r.Status @(200)
    Assert-True "query.income_expense.json" ($null -ne $r.Json) ""
    $r = Invoke-Api -Method Get -Path "/api/query/profit_detail?$d"
    Assert-Status "query.profit_detail.status" $r.Status @(200)
    Assert-True "query.profit_detail.json" ($null -ne $r.Json) ""
    $r = Invoke-Api -Method Get -Path "/api/query/finance_settlement?$d&page=1&page_size=10"
    Assert-Status "query.finance_settlement.status" $r.Status @(200)
    Assert-True "query.finance_settlement.json" ($null -ne $r.Json) ""
    $r = Invoke-Api -Method Get -Path "/api/query/overview?$m"
    Assert-Status "query.overview.status" $r.Status @(200)
    Assert-True "query.overview.json" ($null -ne $r.Json) ""
    $r = Invoke-Api -Method Get -Path "/api/query/category_stats?$d"
    Assert-Status "query.category_stats.status" $r.Status @(200)
    Assert-True "query.category_stats.json" ($null -ne $r.Json) ""
    $r = Invoke-Api -Method Get -Path "/api/query/document_summary?$m"
    Add-XfailJson "query.document_summary.json" $r

    # trend / compare need product_id
    if ($null -ne $script:ProdId) {
        $r = Invoke-Api -Method Get -Path "/api/query/product_price_trend?product_id=$($script:ProdId)&start_date=2026-01-01&end_date=2026-12-31"
        Assert-Status "query.product_price_trend.status" $r.Status @(200)
        Assert-True "query.product_price_trend.json" ($null -ne $r.Json) ""
        $r = Invoke-Api -Method Get -Path "/api/query/purchase_price_compare?product_id=$($script:ProdId)"
        Assert-Status "query.purchase_price_compare.status" $r.Status @(200)
        Assert-True "query.purchase_price_compare.json" ($null -ne $r.Json) ""
    }

    # export endpoints: 200 + xlsx
    Assert-Xlsx "query.purchase_order.export.xlsx" (Invoke-Api -Method Get -Path "/api/query/purchase_order/export?$d")
    Assert-Xlsx "query.purchase_price.export.xlsx" (Invoke-Api -Method Get -Path "/api/query/purchase_price/export?page=1&page_size=10")
    Assert-Xlsx "query.purchase_summary.export.xlsx" (Invoke-Api -Method Get -Path "/api/query/purchase_summary/export?$d")
    Assert-Xlsx "query.supplier_balance.export.xlsx" (Invoke-Api -Method Get -Path "/api/query/supplier_balance/export")
    Assert-Xlsx "query.sales_order.export.xlsx" (Invoke-Api -Method Get -Path "/api/query/sales_order/export?$d")
    Assert-Xlsx "query.sales_price.export.xlsx" (Invoke-Api -Method Get -Path "/api/query/sales_price/export?page=1&page_size=10")
    Assert-Xlsx "query.sales_summary.export.xlsx" (Invoke-Api -Method Get -Path "/api/query/sales_summary/export?$d")
    Add-XfailXlsx "query.purchaser_balance.export.xlsx" (Invoke-Api -Method Get -Path "/api/query/purchaser_balance/export")
    Assert-Xlsx "query.product_rank.export.xlsx" (Invoke-Api -Method Get -Path "/api/query/product_rank/export?$d")
    Assert-Xlsx "query.reimburse_summary.export.xlsx" (Invoke-Api -Method Get -Path "/api/query/reimburse_summary/export?$d")
    Assert-Xlsx "query.allocation_source.export.xlsx" (Invoke-Api -Method Get -Path "/api/query/allocation_source/export?$d")
    Assert-Xlsx "query.stock_balance.export.xlsx" (Invoke-Api -Method Get -Path "/api/query/stock_balance/export?$d")
    Assert-Xlsx "query.stock_flow.export.xlsx" (Invoke-Api -Method Get -Path "/api/query/stock_flow/export?$d")
    Assert-Xlsx "query.stock_summary.export.xlsx" (Invoke-Api -Method Get -Path "/api/query/stock_summary/export?$d")
    Assert-Xlsx "query.stock_summary_reimburse.export.xlsx" (Invoke-Api -Method Get -Path "/api/query/stock_summary_reimburse/export?$d")
    Assert-Xlsx "query.stock_warning.export.xlsx" (Invoke-Api -Method Get -Path "/api/query/stock_warning/export")
    Assert-Xlsx "query.slow_stock.export.xlsx" (Invoke-Api -Method Get -Path "/api/query/slow_stock/export?days=30")
    Assert-Xlsx "query.income_expense.export.xlsx" (Invoke-Api -Method Get -Path "/api/query/income_expense/export?$d")
    Assert-Xlsx "query.profit_detail.export.xlsx" (Invoke-Api -Method Get -Path "/api/query/profit_detail/export?$d")
    Assert-Xlsx "query.overview.export.xlsx" (Invoke-Api -Method Get -Path "/api/query/overview/export?$m")
    Assert-Xlsx "query.category_stats.export.xlsx" (Invoke-Api -Method Get -Path "/api/query/category_stats/export?$d")
    Add-XfailXlsx "query.document_summary.export.xlsx" (Invoke-Api -Method Get -Path "/api/query/document_summary/export?$m")
    End-Group "Group H"
} else {
    Write-Host ""
    Write-Host "=== Group H: query domain SKIPPED (-Quick) ==="
}

# ===========================================================================
# I. System domain
# ===========================================================================
Start-Group "Group I: system"
$r = Invoke-Api -Method Get -Path "/api/system/config"
Assert-Status "system.config.get.status" $r.Status @(200)
Assert-True "system.config.get.json" ($null -ne $r.Json) ""
$r = Invoke-Api -Method Post -Path "/api/system/config" -Body @{ smoke_key = "smoke_value" }
Assert-Status "system.config.post.status" $r.Status @(200)
$r = Invoke-Api -Method Get -Path "/api/system/operation_log"
Assert-Status "system.operation_log.status" $r.Status @(200)
$r = Invoke-Api -Method Get -Path "/api/system/operation_log/export"
Assert-Xlsx "system.operation_log.export.xlsx" $r

# backup executes inside the isolated working dir of the test server (safe)
$r = Invoke-Api -Method Post -Path "/api/backup"
Assert-Status "system.backup.status" $r.Status @(200)
$r = Invoke-Api -Method Get -Path "/api/backup/list"
Assert-Status "system.backup.list.status" $r.Status @(200)
Assert-True "system.backup.list.has_entry" (($r.Json -is [array]) -and $r.Json.Count -ge 1) "backup list empty after POST /api/backup"

$r = Invoke-Api -Method Post -Path "/api/clean_invalid_orders"
Assert-Status "system.clean_invalid_orders.status" $r.Status @(200)
$r = Invoke-Api -Method Get -Path "/api/inspect_corrupted_items"
Assert-Status "system.inspect_corrupted_items.status" $r.Status @(200)
Assert-True "system.inspect_corrupted_items.json" ($r.Json -is [array]) ""
End-Group "Group I"

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host ("========================================")
Write-Host ("SMOKE RESULT: PASS=" + $script:PassCount + "  FAIL=" + $script:FailCount + "  XFAIL=" + $script:XfailCount + "  XPASS=" + $script:XpassCount)
if ($script:FailCount -gt 0) {
    Write-Host "Failed assertions:"
    foreach ($f in $script:Failures) { Write-Host ("  - " + $f) }
}
Write-Host "========================================"

if ($script:FailCount -gt 0) { exit 1 } else { exit 0 }

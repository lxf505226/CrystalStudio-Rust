#Requires -Version 5.1
<#
.SYNOPSIS
    `--shot` 截图确定性门禁（G1 入口）：连跑 N 次，逐字节比 SHA256。

.DESCRIPTION
    用途：证明「同一个 exe + 同一条命令」跑出来的截图**逐字节相同**。

    这条挡的是"截图不可复现"这类缺陷。本仓 2026-11-07 的实测根因是三处**悬停相关输出**
    跟着真实鼠标光标走（画布金色单格描边 / 单元格检查器六行 / 状态栏坐标），
    同一构建同一条命令连跑两次差 2,166 px。修法见：
      * `app::MireditApp::update_hover`（自动化模式不读指针；显式 `--hover` 仍生效）
      * `window::SHOT_WINDOW_POS`（`--shot` 钉窗口位置，隔离显示器/DPI 这一维）
      * `app::MireditApp::handle_screenshot`（等地图列表扫描静默再拍，有上限 + 诊断）

    判定：
      0 = PASS：所有次运行的 PNG SHA256 完全相同
      1 = FAIL：任意两次不同（会打印每张图的字节数 + SHA256，并尽力给出差异像素数与 bbox）
      2 = 用法错误（参数缺失 / 路径不存在 / Runs < 2）

    每次运行用独立目录（同一个 `WorkDir` 下）：
      shot-<i>.png / shot-<i>.stdout.log / shot-<i>.stderr.log / settings-<i>
    设置文件每次运行前删掉 ⇒ 每次都从"没有设置文件"开始，否则上一次的 `recent` / 主题
    会漏进来，那本身就会让像素变（那样 FAIL 就不是被测缺陷，而是夹具不干净）。

    差异像素诊断需要本机有 Python + Pillow；**没有也照样判 PASS/FAIL**，
    只是 FAIL 时退回"首个不同字节偏移"级别的证据。

.PARAMETER Exe
    被测 exe 的完整路径（`miredit-gui.exe`）。**必填**。

.PARAMETER Map
    截图用的 `.map` 路径。**必填**。

.PARAMETER Runs
    连跑几次（最少 2）。缺省 5。

.PARAMETER ExtraArgs
    额外的命令行开关，**每个元素就是一个参数**（原样透传），例如：
      -ExtraArgs '--hover','100,100'
      -ExtraArgs '--show-maps','--frames','60'

.PARAMETER Frames
    透传 `--frames <N>`（第几帧截图）。缺省 0 = 不给这个开关，用程序默认值。

.PARAMETER WorkDir
    产物目录。缺省 `%TEMP%\shot-determinism-<时间戳>`。

.PARAMETER TimeoutSec
    单次运行超时（秒）。缺省 120。超时判 FAIL（一次截图不该跑这么久）。

.PARAMETER Cleanup
    PASS 后删掉产物目录（缺省保留，便于 FAIL 时现场取证）。

.PARAMETER Help
    打印用法后退出，不启动任何进程。

.EXAMPLE
    # 最基本：同一 exe + 同一张图，连跑 5 次
    pwsh -File scripts\shot-determinism.ps1 -Exe .\target\release\miredit-gui.exe -Map E:\maps\0.map

.EXAMPLE
    # A/B 对照：钉住悬停格（显式 --hover 时画面与真实鼠标无关）
    pwsh -File scripts\shot-determinism.ps1 -Exe .\miredit-gui.exe -Map .\0.map -Runs 3 -ExtraArgs '--hover','100,100'

.EXAMPLE
    # 带地图列表面板（扫描中态那条修复的验证场景）
    pwsh -File scripts\shot-determinism.ps1 -Exe .\miredit-gui.exe -Map .\0.map -ExtraArgs '--show-maps','--map-list-root','E:\maps'
#>
[CmdletBinding()]
param(
    [string]$Exe,
    [string]$Map,
    [int]$Runs = 5,
    [string[]]$ExtraArgs = @(),
    [int]$Frames = 0,
    [string]$WorkDir,
    [int]$TimeoutSec = 120,
    [switch]$Cleanup,
    [switch]$Help
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Show-Usage {
    @'
用法：
  pwsh -File scripts\shot-determinism.ps1 -Exe <exe> -Map <map> [-Runs 5] [-ExtraArgs '--hover','x,y']
  pwsh -File scripts\shot-determinism.ps1 -Help

必填：
  -Exe <路径>        被测 exe（miredit-gui.exe）
  -Map <路径>        截图用的 .map

可选：
  -Runs <N>          连跑次数，最少 2，缺省 5
  -ExtraArgs <数组>  透传给 exe 的额外开关，每个元素一个参数
                     例：-ExtraArgs '--hover','100,100'（`pwsh -File` 把逗号数组塌成
                     一个字符串时，脚本会自动还原成逐元素）
  -Frames <N>        透传 --frames（0 = 不给，用程序默认 30）
  -WorkDir <目录>    产物目录（缺省 %TEMP%\shot-determinism-<时间戳>）
  -TimeoutSec <秒>   单次运行超时，缺省 120
  -Cleanup           PASS 后删除产物目录（缺省保留）
  -Help              打印本说明后退出

退出码：0 = PASS（逐字节相同）｜1 = FAIL｜2 = 用法错误
差异像素数与 bbox 需要 Python + Pillow；没有时退回"首个不同字节偏移"，不影响 PASS/FAIL 判定。
'@
}

# 一个 argv 元素 → 命令行片段（Windows 路径里不会有引号，只需要按空白加引号）
function Quote-Arg([string]$s) {
    if ($s -eq '') { return '""' }
    if ($s -match '[\s"]') { return '"' + $s + '"' }
    return $s
}

# `pwsh -File` 会把 `-ExtraArgs '--a','b'` **塌成一个字符串**（实测：COUNT=1，
# 内容是 "'--a','b'"），于是整段会被 exe 当成一个未知开关。这里把"看起来是
# PowerShell 数组字面量"的元素再拆开，让两种调用方式（`& script.ps1 -ExtraArgs a,b`
# 与 `pwsh -File script.ps1 -ExtraArgs 'a','b'`）行为一致。
function Expand-ArrayLiteral([string]$s) {
    if ($s -match "^'.*'(,'.*')+$") {
        return @(($s -split "','") | ForEach-Object { $_.Trim("'") })
    }
    return @($s)
}

# 找一个字节数组首个不同偏移（分块哈希定位 + 块内逐字节，避免在 PS 里扫 5.7 MB）
function Get-FirstDiffOffset([byte[]]$a, [byte[]]$b) {
    $n = [Math]::Min($a.Length, $b.Length)
    $chunk = 4096
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        $off = 0
        while ($off -lt $n) {
            $len = [Math]::Min($chunk, $n - $off)
            $h1 = [Convert]::ToBase64String($sha.ComputeHash($a, $off, $len))
            $h2 = [Convert]::ToBase64String($sha.ComputeHash($b, $off, $len))
            if ($h1 -ne $h2) {
                for ($i = $off; $i -lt ($off + $len); $i++) {
                    if ($a[$i] -ne $b[$i]) { return $i }
                }
            }
            $off += $len
        }
        if ($a.Length -ne $b.Length) { return $n }
        return -1
    } finally {
        $sha.Dispose()
    }
}

# 差异像素诊断：优先 Python + Pillow；不可用则返回 $null（调用方退回字节级证据）
function Get-PixelDiff([string]$A, [string]$B, [string]$Engine) {
    if (-not $Engine) { return $null }
    $out = & $Engine $script:PixelDiffPy $A $B 2>$null
    if ($LASTEXITCODE -ne 0) { return $null }
    return ($out | Select-Object -Last 1)
}

# 找可用的 Python（有 Pillow 才算）
function Find-PillowPython {
    $cands = @()
    foreach ($name in @('python', 'python3', 'py')) {
        $cmd = Get-Command $name -ErrorAction SilentlyContinue
        if ($cmd) { $cands += $cmd.Source }
    }
    foreach ($py in $cands) {
        & $py -c "import PIL" 2>$null | Out-Null
        if ($LASTEXITCODE -eq 0) { return $py }
    }
    return $null
}

if ($Help) {
    Show-Usage
    exit 0
}

if (-not $Exe -or -not $Map) {
    Write-Host '用法错误：-Exe 与 -Map 都是必填（用 -Help 看全部开关）。'
    exit 2
}
if ($Runs -lt 2) {
    Write-Host "用法错误：-Runs 至少要 2（拿到的是 $Runs；只跑一次没有"两次相同"可言）。"
    exit 2
}
if (-not (Test-Path -LiteralPath $Exe -PathType Leaf)) {
    Write-Host "用法错误：-Exe 不存在或不是文件：$Exe"
    exit 2
}
if (-not (Test-Path -LiteralPath $Map -PathType Leaf)) {
    Write-Host "用法错误：-Map 不存在或不是文件：$Map"
    exit 2
}
$Exe = (Resolve-Path -LiteralPath $Exe).ProviderPath
$Map = (Resolve-Path -LiteralPath $Map).ProviderPath

# 见 Expand-ArrayLiteral：把 `pwsh -File` 塌掉的那个数组还原回逐元素
if ($ExtraArgs.Count -gt 0) {
    $ExtraArgs = @($ExtraArgs | ForEach-Object { Expand-ArrayLiteral $_ })
}

if (-not $WorkDir) {
    $WorkDir = Join-Path $env:TEMP ('shot-determinism-' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
}
$WorkDir = [System.IO.Path]::GetFullPath($WorkDir)
New-Item -ItemType Directory -Force -Path $WorkDir | Out-Null

Write-Host ('[截图确定性] exe = {0}' -f $Exe)
Write-Host ('[截图确定性] map = {0}' -f $Map)
Write-Host ('[截图确定性] 次数 = {0} / 产物目录 = {1}' -f $Runs, $WorkDir)
if ($ExtraArgs.Count -gt 0) {
    Write-Host ('[截图确定性] 额外开关 = {0}' -f ($ExtraArgs -join ' '))
}

# ── 逐次运行 ────────────────────────────────────────────────────────────────
$shots = New-Object System.Collections.Generic.List[string]
for ($i = 1; $i -le $Runs; $i++) {
    $shot = Join-Path $WorkDir ('shot-{0}.png' -f $i)
    $settings = Join-Path $WorkDir ('settings-{0}' -f $i)
    $outLog = Join-Path $WorkDir ('shot-{0}.stdout.log' -f $i)
    $errLog = Join-Path $WorkDir ('shot-{0}.stderr.log' -f $i)
    # 夹具干净：截图与设置文件都从"不存在"开始（见 .DESCRIPTION）
    foreach ($p in @($shot, $settings, $outLog, $errLog)) {
        if (Test-Path -LiteralPath $p) { Remove-Item -LiteralPath $p -Force }
    }

    $argv = @('--map', $Map, '--shot', $shot, '--settings', $settings)
    if ($Frames -gt 0) { $argv += @('--frames', [string]$Frames) }
    if ($ExtraArgs.Count -gt 0) { $argv += $ExtraArgs }
    $argLine = ($argv | ForEach-Object { Quote-Arg $_ }) -join ' '

    Write-Host ('[run {0}/{1}] 启动 …' -f $i, $Runs)
    $proc = Start-Process -FilePath $Exe -ArgumentList $argLine -NoNewWindow -PassThru `
        -RedirectStandardOutput $outLog -RedirectStandardError $errLog
    $ms = [int]([Math]::Max(1, $TimeoutSec) * 1000)
    $exited = $proc.WaitForExit($ms)
    if (-not $exited) {
        try { $proc.Kill() } catch { }
        Write-Host ('FAIL：第 {0} 次运行超时（>{1}s，已强杀）。stderr 尾部：' -f $i, $TimeoutSec)
        if (Test-Path -LiteralPath $errLog) { Get-Content -LiteralPath $errLog -Tail 20 | ForEach-Object { Write-Host "  $_" } }
        exit 1
    }
    if ($proc.ExitCode -ne 0) {
        Write-Host ('FAIL：第 {0} 次退出码 {1}。stderr 尾部：' -f $i, $proc.ExitCode)
        if (Test-Path -LiteralPath $errLog) { Get-Content -LiteralPath $errLog -Tail 20 | ForEach-Object { Write-Host "  $_" } }
        exit 1
    }
    if (-not (Test-Path -LiteralPath $shot -PathType Leaf)) {
        Write-Host ('FAIL：第 {0} 次没有生成截图：{1}。stderr 尾部：' -f $i, $shot)
        if (Test-Path -LiteralPath $errLog) { Get-Content -LiteralPath $errLog -Tail 20 | ForEach-Object { Write-Host "  $_" } }
        exit 1
    }
    $shots.Add($shot)
}

# ── 逐字节比对 ──────────────────────────────────────────────────────────────
$rows = foreach ($s in $shots) {
    $f = Get-Item -LiteralPath $s
    [pscustomobject]@{
        File  = [System.IO.Path]::GetFileName($s)
        Bytes = $f.Length
        Hash  = (Get-FileHash -LiteralPath $s -Algorithm SHA256).Hash
    }
}
Write-Host ''
Write-Host '序号  字节数      SHA256'
$k = 0
foreach ($r in $rows) {
    $k++
    Write-Host ('{0,-5} {1,-11} {2}' -f $k, $r.Bytes, $r.Hash)
}

$uniq = @($rows | Select-Object -ExpandProperty Hash | Select-Object -Unique)
if ($uniq.Count -eq 1) {
    Write-Host ''
    Write-Host ('PASS：{0}/{0} 逐字节相同（SHA256 {1}）' -f $Runs, $uniq[0])
    if ($Cleanup) {
        Remove-Item -LiteralPath $WorkDir -Recurse -Force
        Write-Host '已按 -Cleanup 删除产物目录。'
    } else {
        Write-Host ('产物目录（保留）：{0}' -f $WorkDir)
    }
    exit 0
}

# ── FAIL：给差异证据 ────────────────────────────────────────────────────────
Write-Host ''
Write-Host ('FAIL：{0} 次运行里有 {1} 种不同字节 —— 这条命令的截图不可复现。' -f $Runs, $uniq.Count)

# 差异像素诊断需要 Python + Pillow（没有就退回字节级证据，判定不受影响）
$script:PixelDiffPy = Join-Path $WorkDir 'pixel-diff.py'
$pyEngine = Find-PillowPython
if ($pyEngine) {
    $pyCode = @'
import sys
from PIL import Image

a = Image.open(sys.argv[1]).convert("RGBA")
b = Image.open(sys.argv[2]).convert("RGBA")
if a.size != b.size:
    print("SIZE %dx%d %dx%d" % (a.width, a.height, b.width, b.height))
    sys.exit(0)
w, h = a.size
pa = a.tobytes()
pb = b.tobytes()
n = 0
x0 = y0 = 1 << 30
x1 = y1 = -1
maxd = 0
for i in range(0, len(pa), 4):
    if pa[i:i + 4] != pb[i:i + 4]:
        p = i >> 2
        x = p % w
        y = p // w
        n += 1
        if x < x0:
            x0 = x
        if y < y0:
            y0 = y
        if x > x1:
            x1 = x
        if y > y1:
            y1 = y
        d = max(abs(pa[i] - pb[i]), abs(pa[i + 1] - pb[i + 1]),
                abs(pa[i + 2] - pb[i + 2]), abs(pa[i + 3] - pb[i + 3]))
        if d > maxd:
            maxd = d
print("DIFF %d BBOX %d,%d,%d,%d SIZE %dx%d MAXDELTA %d" % (n, x0, y0, x1, y1, w, h, maxd))
'@
    Set-Content -LiteralPath $script:PixelDiffPy -Value $pyCode -Encoding ASCII
} else {
    Write-Host '提示：本机没有 Python+Pillow ⇒ 只给字节级差异（像素数不可用，不影响 PASS/FAIL）。'
}

$first = $shots[0]
$firstBytes = [System.IO.File]::ReadAllBytes($first)
for ($i = 1; $i -lt $shots.Count; $i++) {
    $other = $shots[$i]
    Write-Host ''
    Write-Host ('第 1 张 vs 第 {0} 张：' -f ($i + 1))
    $px = Get-PixelDiff $first $other $pyEngine
    if ($px) {
        if ($px -match '^SIZE ') {
            Write-Host ('  尺寸不同：{0}' -f $px.Substring(5))
        } elseif ($px -match '^DIFF (\d+) BBOX (-?\d+),(-?\d+),(-?\d+),(-?\d+) SIZE (\d+)x(\d+) MAXDELTA (\d+)$') {
            $n = [int]$Matches[1]
            $bx0 = [int]$Matches[2]; $by0 = [int]$Matches[3]
            $bx1 = [int]$Matches[4]; $by1 = [int]$Matches[5]
            $w = [int]$Matches[6]; $hh = [int]$Matches[7]
            $md = [int]$Matches[8]
            if ($n -eq 0) {
                Write-Host '  像素完全相同（差异只在 PNG 编码层：元数据/压缩参数）'
            } else {
                Write-Host ('  差异像素 {0} / {1}（图 {2}×{3}），bbox x {4}..{5}，y {6}..{7}（{8}×{9}），最大通道差 {10}' -f `
                        $n, ($w * $hh), $w, $hh, $bx0, $bx1, $by0, $by1, ($bx1 - $bx0 + 1), ($by1 - $by0 + 1), $md)
            }
        } else {
            Write-Host ('  像素诊断输出无法解析：{0}' -f $px)
        }
    } else {
        $b1 = [System.IO.File]::ReadAllBytes($other)
        if ($firstBytes.Length -ne $b1.Length) {
            Write-Host ('  字节数不同：{0} vs {1}' -f $firstBytes.Length, $b1.Length)
        } else {
            $off = Get-FirstDiffOffset $firstBytes $b1
            Write-Host ('  字节数相同（{0}），首个不同偏移 0x{1:X}（第 {1} 字节）' -f $b1.Length, $off)
        }
    }
}

Write-Host ''
Write-Host '各次 stderr（含扫描中态诊断等）都在：'
foreach ($s in $shots) {
    $log = [System.IO.Path]::ChangeExtension($s, '.stderr.log')
    Write-Host ('  {0}' -f $log)
}
Write-Host ('产物目录（保留，便于取证）：{0}' -f $WorkDir)
exit 1
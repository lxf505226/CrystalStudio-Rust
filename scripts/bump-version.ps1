#Requires -Version 5.1

<#
.SYNOPSIS
    版本号校验与递增。唯一真源 = 根 Cargo.toml 的 [workspace.package].version。

.DESCRIPTION
    三种用法（互斥）：

      -Check
          只读校验：根 [workspace.package].version 可读且是合法 SemVer；crates/ 下 8 个 crate
          都是 version.workspace = true；vendor/wil2atlas 作为 vendored 上游例外单独报（不参与比对）。
          一致 ⇒ 退出码 0，不一致 ⇒ 退出码 1。

      -Part patch|minor|major
          按当前版本递增：patch 只升 Z；minor 升 Y 且 Z 归零；major 在 1.0 之前被拒绝
          （0.x 阶段用 minor 表示 breaking），1.0 之后按正常 SemVer 升 X 并归零 Y/Z。
          0.x 阶梯（VERSIONING.md §5.1）：0.0.1 -Part patch ⇒ 0.0.2（只修不增）；
          0.0.1 -Part minor ⇒ 0.1.0（首个 MINOR 功能里程碑）。

      -Set X.Y.Z
          显式设成指定三段版本（严格 SemVer 校验；-rc.N / +g<sha> 由发布流程另定，本脚本不处理）。

    -Part / -Set 只做两件事：
      1) 改根 Cargo.toml 的 [workspace.package].version（只改这一行，别的文件不碰）；
      2) 把 CHANGELOG.md 的 `## [Unreleased]` 改成 `## [X.Y.Z] - <系统当天日期>`，
         并在顶部重建一个空的 `## [Unreleased]`。
    脚本不伪造变更条目、不碰 git、不打 tag、不跑测试；写完会复读复核，最后打印发布清单。

    规则出处：VERSIONING.md §5.1（档位判据）、§5.2（判定算法）、§5.3（本脚本）、§六（发布清单）。

.PARAMETER Check
    只校验，不写盘。

.PARAMETER Part
    递增哪一段：patch / minor / major。

.PARAMETER Set
    目标版本号，形如 1.2.3。

.PARAMETER Root
    项目根目录。缺省 = 本脚本所在目录（scripts/）的上一级。仅供从别处调用或用夹具测试时使用。

.EXAMPLE
    pwsh -File scripts/bump-version.ps1 -Check

.EXAMPLE
    pwsh -File scripts/bump-version.ps1 -Part minor

.EXAMPLE
    pwsh -File scripts/bump-version.ps1 -Set 0.1.0

.NOTES
    退出码：0 = 成功；1 = 校验失败或前置条件不满足（含「根 Cargo.toml 缺 [workspace.package]」）。
    文件编码：读写一律 UTF-8 无 BOM，换行沿用文件里已有的风格（本仓是 LF）。
#>
[CmdletBinding(DefaultParameterSetName = 'Check')]
param(
    [Parameter(ParameterSetName = 'Check')]
    [switch]$Check,

    [Parameter(ParameterSetName = 'Part', Mandatory = $true)]
    [ValidateSet('patch', 'minor', 'major')]
    [string]$Part,

    [Parameter(ParameterSetName = 'Set', Mandatory = $true)]
    [string]$Set,

    [Parameter(ParameterSetName = 'Check')]
    [Parameter(ParameterSetName = 'Part')]
    [Parameter(ParameterSetName = 'Set')]
    [string]$Root
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# ───────────────────────────── 常量 ─────────────────────────────

# 根 Cargo.toml 里放版本号的那张表
$WorkspaceTable = 'workspace.package'

# crates/ 下应有的 crate 数量（增删 crate 时同步 VERSIONING.md §3.1 与这里）
$ExpectedCrateCount = 8

# vendored 的上游解码器子集：保留它自己的版本，不随本仓 bump（VERSIONING.md §二/§3.1）
$VendoredManifest = 'vendor/wil2atlas/Cargo.toml'

# 严格三段 SemVer：不写前导零，不接受 -rc.N / +g<sha>
$SemVerCorePattern = '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$'

# 新 [Unreleased] 下的占位说明。逐字比对：下次 bump 时只清掉这一行，人写的备注一律保留。
$UnreleasedPlaceholder = '> 暂无条目。新改动先记在这里，发布时整体搬进下一个版本标题。'

# 统一 stdout 输出（避免 Write-Host 的 information 流在重定向时行为不一致）
function Write-Line {
    param([string]$Text = '')
    [Console]::Out.WriteLine($Text)
}

# 失败统一走这里：打印原因 + 退出码 1。绝不在失败路径上继续写盘。
function Stop-Failure {
    param(
        [string]$Title,
        [string[]]$Detail = @()
    )
    Write-Line ''
    Write-Line "❌ $Title"
    foreach ($d in $Detail) { Write-Line "   $d" }
    Write-Line ''
    exit 1
}

# ───────────────────────────── 文件读写（UTF-8 无 BOM） ─────────────────────────────

function Get-FileText {
    param([string]$Path)
    return [System.IO.File]::ReadAllText($Path)
}

function Save-FileText {
    param([string]$Path, [string]$Text)
    $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $Text, $utf8NoBom)
}

# 沿用文件既有换行风格，避免整文件 diff 噪声
function Get-Newline {
    param([string]$Text)
    if ($Text.Contains("`r`n")) { return "`r`n" }
    return "`n"
}

# 写盘后自证没有 BOM（本仓全部文件是 UTF-8 无 BOM）
function Assert-NoBom {
    param([string]$Path)
    $bytes = [System.IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -ge 3 -and $bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB -and $bytes[2] -eq 0xBF) {
        Stop-Failure "写入后复核失败：$Path 带了 UTF-8 BOM" '本仓要求 UTF-8 无 BOM；请检查编辑器设置，勿提交本次改动。'
    }
}

# ───────────────────────────── TOML 读取（只认当前仓库用到的两种写法） ─────────────────────────────

# 取指定表里的 version 键。返回 $null（表或键不存在），或：
#   Kind = 'literal'    ⇒ version = "0.0.1"（写死，违反单一真源）
#   Kind = 'workspace'  ⇒ version.workspace = true（正确写法）
#   Value = 版本号 / 'workspace'；Line = 行号（1 起）
function Get-TomlVersionEntry {
    param([string]$Text, [string]$Table)

    $lines = $Text -split "`r?`n"
    $headerRe = '^\s*\[\s*' + [regex]::Escape($Table) + '\s*\]\s*$'
    $anyHeaderRe = '^\s*\['
    $inSection = $false

    for ($i = 0; $i -lt $lines.Count; $i++) {
        $line = $lines[$i]

        if ($line -match $anyHeaderRe) {
            # 进入新表（含 [[bin]]）：只有表头恰好是目标表时才开始收集
            $inSection = [bool]($line -match $headerRe)
            continue
        }
        if (-not $inSection) { continue }

        if ([regex]::IsMatch($line, '^\s*version\s*\.\s*workspace\s*=\s*true\s*(#.*)?$')) {
            return [pscustomobject]@{ Kind = 'workspace'; Value = 'workspace'; Line = $i + 1 }
        }
        $m = [regex]::Match($line, '^\s*version\s*=\s*"([^"]*)"\s*(#.*)?$')
        if ($m.Success) {
            return [pscustomobject]@{ Kind = 'literal'; Value = $m.Groups[1].Value; Line = $i + 1 }
        }
    }
    return $null
}

# ───────────────────────────── SemVer ─────────────────────────────

function Test-SemVerCore {
    param([string]$Text)
    return [regex]::IsMatch($Text, $SemVerCorePattern)
}

function Get-SemVerParts {
    param([string]$Text)
    $m = [regex]::Match($Text, $SemVerCorePattern)
    if (-not $m.Success) { return $null }
    return [pscustomobject]@{
        Major = [int]$m.Groups[1].Value
        Minor = [int]$m.Groups[2].Value
        Patch = [int]$m.Groups[3].Value
    }
}

function Get-NextVersion {
    param([string]$Current, [string]$Kind)

    $v = Get-SemVerParts $Current
    if ($null -eq $v) { Stop-Failure "当前版本号 '$Current' 不是合法 SemVer，拒绝递增" }

    switch ($Kind) {
        'patch' { return "$($v.Major).$($v.Minor).$($v.Patch + 1)" }
        'minor' { return "$($v.Major).$($v.Minor + 1).0" }
        'major' {
            if ($v.Major -eq 0) {
                Stop-Failure '1.0 之前不能用 major' @(
                    "当前版本 $Current 还在 0.x：0.x 阶段请用 -Part minor 表示 breaking（VERSIONING.md §5.2）。",
                    'major 只在 1.0 之后用来表示「破坏已发布的兼容性」。'
                )
            }
            return "$($v.Major + 1).0.0"
        }
        default { Stop-Failure "未知的递增段 '$Kind'" }
    }
}

# ───────────────────────────── 前置校验 ─────────────────────────────

function Get-SingleSourceHint {
    return @(
        '',
        '先落地单一真源（VERSIONING.md §3.1 / §六 第 1 步）：',
        '  1) 根 Cargo.toml 增加一段',
        '       [workspace.package]',
        '       version = "<当前对外版本>"',
        '  2) crates/ 下 8 个 crate 的 [package] 从 version = "x.y.z" 改成',
        '       version.workspace = true',
        '  3) 再跑一次 -Check，直到全绿。',
        '本脚本不会替你改 crates/*/Cargo.toml，也不会猜一个版本号写进根 Cargo.toml。'
    )
}

# 返回 @{ Problems = [List[string]]; Info = <见下> }，Problems 为空才算通过
function Invoke-Precheck {
    param([string]$RootPath)

    $problems = New-Object System.Collections.Generic.List[string]
    $info = [pscustomobject]@{
        RootManifest    = Join-Path $RootPath 'Cargo.toml'
        RootVersion     = $null
        RootLine        = 0
        Crates          = @()
        VendoredVersion = $null
    }

    if (-not (Test-Path -LiteralPath $info.RootManifest -PathType Leaf)) {
        $problems.Add("找不到根 Cargo.toml：$($info.RootManifest)")
        return [pscustomobject]@{ Problems = $problems; Info = $info }
    }

    # ① 根：唯一真源
    $rootText = Get-FileText $info.RootManifest
    $entry = Get-TomlVersionEntry -Text $rootText -Table $WorkspaceTable
    if ($null -eq $entry) {
        $problems.Add("根 Cargo.toml 没有 [$WorkspaceTable].version —— 版本号还没有唯一真源")
    }
    elseif ($entry.Kind -ne 'literal') {
        $problems.Add("根 Cargo.toml 的 [$WorkspaceTable].version 不是字符串版本号（第 $($entry.Line) 行）")
    }
    else {
        $info.RootVersion = $entry.Value
        $info.RootLine = $entry.Line
        if (-not (Test-SemVerCore $entry.Value)) {
            $problems.Add("根 Cargo.toml:$($entry.Line) 的 version = ""$($entry.Value)"" 不是合法三段 SemVer")
        }
    }

    # ② crates/*：必须引用 workspace 版本
    $cratesDir = Join-Path $RootPath 'crates'
    $crates = @()
    if (-not (Test-Path -LiteralPath $cratesDir -PathType Container)) {
        $problems.Add("找不到 crates/ 目录：$cratesDir")
    }
    else {
        $dirs = @(Get-ChildItem -LiteralPath $cratesDir -Directory | Sort-Object Name)
        if ($dirs.Count -ne $ExpectedCrateCount) {
            $problems.Add("crates/ 下应有 $ExpectedCrateCount 个 crate，实测 $($dirs.Count) 个（增删 crate 时请同步 VERSIONING.md §3.1 与本脚本的期望值）")
        }
        foreach ($d in $dirs) {
            $manifest = Join-Path $d.FullName 'Cargo.toml'
            $item = [pscustomobject]@{ Name = $d.Name; Kind = 'missing'; Value = ''; Line = 0 }

            if (-not (Test-Path -LiteralPath $manifest -PathType Leaf)) {
                $problems.Add("crates/$($d.Name)/Cargo.toml 不存在")
                $crates += $item
                continue
            }

            $text = Get-FileText $manifest
            $e = Get-TomlVersionEntry -Text $text -Table 'package'
            if ($null -eq $e) {
                $problems.Add("crates/$($d.Name)/Cargo.toml 的 [package] 里没有 version 键")
                $crates += $item
                continue
            }

            $item.Kind = $e.Kind
            $item.Value = $e.Value
            $item.Line = $e.Line
            if ($e.Kind -ne 'workspace') {
                $problems.Add("crates/$($d.Name)/Cargo.toml:$($e.Line) 写死 version = ""$($e.Value)""，应改为 version.workspace = true")
            }
            $crates += $item
        }
    }
    $info.Crates = $crates

    # ③ vendored 上游：例外，只报告不比对
    $vendoredPath = Join-Path $RootPath ($VendoredManifest -replace '/', [System.IO.Path]::DirectorySeparatorChar)
    if (Test-Path -LiteralPath $vendoredPath -PathType Leaf) {
        $e = Get-TomlVersionEntry -Text (Get-FileText $vendoredPath) -Table 'package'
        if ($null -ne $e -and $e.Kind -eq 'literal') { $info.VendoredVersion = $e.Value }
    }
    else {
        Write-Line "⚠ 未找到 vendored 上游 $VendoredManifest（跳过，不影响本次校验）"
    }

    return [pscustomobject]@{ Problems = $problems; Info = $info }
}

# ───────────────────────────── -Check ─────────────────────────────

function Invoke-Check {
    param([string]$RootPath)

    $pre = Invoke-Precheck -RootPath $RootPath
    $info = $pre.Info

    Write-Line "[检查] 项目根：$RootPath"
    Write-Line "[检查] 根 Cargo.toml：$($info.RootManifest)"
    if ($null -ne $info.RootVersion) {
        Write-Line "[检查] 当前版本（[$WorkspaceTable].version，第 $($info.RootLine) 行）：$($info.RootVersion)"
    }
    else {
        Write-Line "[检查] 当前版本（[$WorkspaceTable].version）：<缺失>"
    }

    Write-Line "[检查] crates/ 下共 $($info.Crates.Count) 个 crate（期望 $ExpectedCrateCount 个）："
    foreach ($c in $info.Crates) {
        switch ($c.Kind) {
            'workspace' { Write-Line "   ✅ $($c.Name)：version.workspace = true" }
            'literal' { Write-Line "   ❌ $($c.Name)：写死 version = ""$($c.Value)""（第 $($c.Line) 行）" }
            default { Write-Line "   ❌ $($c.Name)：Cargo.toml 缺失或 [package] 无 version" }
        }
    }

    $vendoredText = if ($null -ne $info.VendoredVersion) { $info.VendoredVersion } else { '<未读到>' }
    Write-Line "[检查] $VendoredManifest：version = $vendoredText（vendored 上游解码器，保留自身上游版本，不随本仓 bump）"

    if ($pre.Problems.Count -eq 0) {
        Write-Line ''
        Write-Line "✅ 一致：版本号唯一真源 = 根 Cargo.toml 的 [$WorkspaceTable].version = $($info.RootVersion)，8 个 crate 全部引用它。"
        Write-Line '[检查] 退出码 0'
        return 0
    }

    Write-Line ''
    Write-Line "❌ 不一致：$($pre.Problems.Count) 处问题"
    foreach ($p in $pre.Problems) { Write-Line "   - $p" }
    foreach ($h in (Get-SingleSourceHint)) { Write-Line $h }
    Write-Line '[检查] 退出码 1'
    return 1
}

# ───────────────────────────── 生成新文本（纯计算，不写盘） ─────────────────────────────

# 根 Cargo.toml：只把 [workspace.package] 里的那一行 version 换成新值，其余逐字节照抄
function New-WorkspaceVersionText {
    param(
        [string]$Text,
        [int]$LineNumber,
        [string]$OldVersion,
        [string]$NewVersion
    )

    $nl = Get-Newline $Text
    $lines = @($Text -split "`r?`n")

    if ($LineNumber -lt 1 -or $LineNumber -gt $lines.Count) {
        Stop-Failure "根 Cargo.toml 的版本行号越界（$LineNumber），拒绝写入"
    }

    $line = $lines[$LineNumber - 1]
    $pattern = '^(\s*version\s*=\s*")' + [regex]::Escape($OldVersion) + '(")'
    if (-not [regex]::IsMatch($line, $pattern)) {
        # 落盘前断言内容特征：不匹配就停手，绝不盲写
        Stop-Failure "根 Cargo.toml:$LineNumber 已不是预期的版本行，拒绝写入（避免改错文件）" @("实际内容：$line")
    }

    $replacement = '${1}' + $NewVersion + '${2}'
    $lines[$LineNumber - 1] = [regex]::Replace($line, $pattern, $replacement)
    return ($lines -join $nl)
}

# CHANGELOG.md：把 `## [Unreleased]` 搬成 `## [X.Y.Z] - 今天`，并在最上面重建空的 `## [Unreleased]`
# 返回 @{ Text = 新全文; HadEntries = 这次是否真有条目; Today = 用到的日期 }
# 纪律：只搬标题、只清掉脚本自己写的那句占位说明，绝不编造/改动任何变更条目
function New-ChangelogText {
    param([string]$Text, [string]$Version)

    $nl = Get-Newline $Text
    $lines = @($Text -split "`r?`n")

    # 守卫 ①：目标版本小节已存在 ⇒ 拒绝，避免出现两个同名标题或覆盖上一版
    if ([regex]::IsMatch($Text, '(?m)^##\s*\[' + [regex]::Escape($Version) + '\]')) {
        Stop-Failure "CHANGELOG.md 里已经有 [$Version] 小节，拒绝重复创建（本次未改动任何文件）" @(
            '先确认是不是重复执行；要改已发布的版本标题请手工编辑（脚本不覆盖已发布内容）。'
        )
    }

    # 守卫 ②：`## [Unreleased]` 必须恰好一个，脚本不猜哪个才是要发布的那一段
    $unreleasedIdx = -1
    $hits = 0
    for ($i = 0; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match '^##\s*\[Unreleased\]\s*$') {
            $hits++
            $unreleasedIdx = $i
        }
    }
    if ($hits -ne 1) {
        Stop-Failure "CHANGELOG.md 里 '## [Unreleased]' 标题应恰好 1 个，实测 $hits 个（本次未改动任何文件）" @(
            '脚本不判断哪个才该发布（VERSIONING.md §九 第 3 条）；请先把标题收敛成一个再跑。'
        )
    }

    # 截出 [Unreleased] 的正文：标题下一行到下一个 `## [` 之前
    $nextIdx = $lines.Count
    for ($j = $unreleasedIdx + 1; $j -lt $lines.Count; $j++) {
        if ($lines[$j] -match '^##\s*\[') { $nextIdx = $j; break }
    }

    $body = New-Object System.Collections.Generic.List[string]
    if ($nextIdx -gt $unreleasedIdx + 1) {
        foreach ($i in ($unreleasedIdx + 1)..($nextIdx - 1)) { $body.Add($lines[$i]) }
    }

    # 只清掉脚本自己写的那句占位说明（逐字比对），人写的备注一律保留
    for ($i = $body.Count - 1; $i -ge 0; $i--) {
        if ($body[$i].Trim() -eq $UnreleasedPlaceholder) { $body.RemoveAt($i) }
    }
    while ($body.Count -gt 0 -and $body[0] -match '^\s*$') { $body.RemoveAt(0) }
    while ($body.Count -gt 0 -and $body[$body.Count - 1] -match '^\s*$') { $body.RemoveAt($body.Count - 1) }

    # 「暂无条目」类说明（本仓 [Unreleased] 下常见）会被搬进版本标题下；
    # 脚本不敢替人判断它算不算本版内容，只报出来让人核对。
    $bodyHasNoneNote = $false
    foreach ($l in $body) { if ($l -match '^>\s*暂无条目') { $bodyHasNoneNote = $true; break } }

    $hadEntries = ($body.Count -gt 0)
    $today = (Get-Date).ToString('yyyy-MM-dd', [System.Globalization.CultureInfo]::InvariantCulture)

    $out = New-Object System.Collections.Generic.List[string]
    if ($unreleasedIdx -gt 0) {
        foreach ($i in 0..($unreleasedIdx - 1)) { $out.Add($lines[$i]) }
        while ($out.Count -gt 0 -and $out[$out.Count - 1] -match '^\s*$') { $out.RemoveAt($out.Count - 1) }
    }

    $out.Add('')
    $out.Add('## [Unreleased]')
    $out.Add('')
    $out.Add($UnreleasedPlaceholder)
    $out.Add('')
    $out.Add("## [$Version] - $today")
    if ($hadEntries) {
        $out.Add('')
        foreach ($l in $body) { $out.Add($l) }
    }

    $out.Add('')
    if ($nextIdx -lt $lines.Count) {
        foreach ($i in $nextIdx..($lines.Count - 1)) { $out.Add($lines[$i]) }
    }
    # 末尾只留一个换行（去干净尾部空行，再补一个空串让 join 产生结尾 "\n"）
    while ($out.Count -gt 0 -and $out[$out.Count - 1] -match '^\s*$') { $out.RemoveAt($out.Count - 1) }
    $out.Add('')

    return [pscustomobject]@{
        Text           = ($out -join $nl)
        HadEntries     = $hadEntries
        HasNoneNote    = $bodyHasNoneNote
        Today          = $today
    }
}

# ───────────────────────────── 形状断言 + 落盘（失败回滚，不留半写） ─────────────────────────────

# 断言：CHANGELOG 文本里恰有一个带日期的目标版本标题、恰有一个 [Unreleased]
function Assert-ChangelogShape {
    param([string]$Text, [string]$Version, [string]$Context, [string]$Hint)

    $versionHeadings = [regex]::Matches($Text, '(?m)^##\s*\[' + [regex]::Escape($Version) + '\]\s*-\s*\d{4}-\d{2}-\d{2}\s*$').Count
    if ($versionHeadings -ne 1) {
        Stop-Failure "$Context：CHANGELOG 里 '## [$Version] - YYYY-MM-DD' 标题应有 1 个，实测 $versionHeadings 个" @($Hint)
    }
    $unreleasedHeadings = [regex]::Matches($Text, '(?m)^##\s*\[Unreleased\]\s*$').Count
    if ($unreleasedHeadings -ne 1) {
        Stop-Failure "$Context：CHANGELOG 里 '## [Unreleased]' 应有 1 个，实测 $unreleasedHeadings 个" @($Hint)
    }
}

# 两份内容都算完、都自检通过后才落盘；任一文件写失败就把另一份写回原内容（不留半写）
function Commit-Bump {
    param(
        [string]$RootManifest,
        [string]$RootOriginalText,
        [string]$RootNewText,
        [string]$ChangelogPath,
        [string]$ChangelogOriginalText,
        [string]$ChangelogNewText
    )

    $rootWritten = $false
    try {
        Save-FileText -Path $RootManifest -Text $RootNewText
        $rootWritten = $true
        Save-FileText -Path $ChangelogPath -Text $ChangelogNewText
    }
    catch {
        if ($rootWritten) { Save-FileText -Path $RootManifest -Text $RootOriginalText }
        Save-FileText -Path $ChangelogPath -Text $ChangelogOriginalText
        Stop-Failure '写盘失败，已把两个文件都写回修改前的内容' @("原因：$($_.Exception.Message)")
    }

    Assert-NoBom -Path $RootManifest
    Assert-NoBom -Path $ChangelogPath
}

# 写入后复核：从磁盘读回来再验一遍（不信任内存里的结果）
function Assert-PostWrite {
    param([string]$RootPath, [string]$ExpectedVersion)

    $rootManifest = Join-Path $RootPath 'Cargo.toml'
    $e = Get-TomlVersionEntry -Text (Get-FileText $rootManifest) -Table $WorkspaceTable
    if ($null -eq $e -or $e.Kind -ne 'literal' -or $e.Value -ne $ExpectedVersion) {
        Stop-Failure "写入后复核失败：根 [$WorkspaceTable].version 读回不是 $ExpectedVersion" @(
            '这是脚本自身的缺陷，请勿提交本次改动。'
        )
    }

    Assert-ChangelogShape -Text (Get-FileText (Join-Path $RootPath 'CHANGELOG.md')) `
        -Version $ExpectedVersion -Context '写入后复核' `
        -Hint '这是脚本自身的缺陷，请勿提交本次改动。'
}

# ───────────────────────────── 发布清单 ─────────────────────────────

function Show-ReleaseChecklist {
    param([string]$Version, [string]$OldVersion)

    Write-Line ''
    Write-Line '──────────────────────── 发布清单（VERSIONING.md §六，按顺序执行） ────────────────────────'
    Write-Line '1. 门禁三条（必须全绿；本仓不跑 cargo fmt）：'
    Write-Line '     cargo test --workspace'
    Write-Line '     cargo clippy --all-targets -- -D warnings'
    Write-Line '     cargo check --workspace'
    Write-Line '   ⚠ 没设 MIREDIT_ASSET_ROOT / MIREDIT_LIB_ROOT 时素材用例打印 [skip] 也算绿，但发布说明要讲清是哪种。'
    Write-Line '2. 版本展示面核对：'
    Write-Line "     cargo pkgid -p miredit-web                  # 末段应为 #$Version"
    Write-Line '     curl -s http://127.0.0.1:8080/api/health    # version 必须等于新版本'
    Write-Line '     miredit-gui --version                       # 必须输出 Crystal Studio <版本>（VERSIONING.md §3.2）；「关于」对话框若已落地，版本串必须与这里一致'
    Write-Line '3. 干净构建（一次性 target 目录，别塞进仓库树）：'
    Write-Line '     CARGO_TARGET_DIR=<独立目录> cargo build --release --workspace'
    Write-Line "     git archive --format=zip --prefix=CrystalStudio-$Version/ v$Version -o CrystalStudio-$Version.zip"
    Write-Line '4. 提交并打 tag（tag 打在发布提交上，不是之前的提交）：'
    Write-Line '     git add -A'
    Write-Line ('     git commit -m "发布 v{0}：<一句话摘要>"' -f $Version)
    Write-Line ('     git tag -a v{0} -m "Crystal Studio v{0}"' -f $Version)
    Write-Line '     git push origin main --tags'
    Write-Line "5. GitHub Release：仓 lxf505226/CrystalStudio-Rust，标题 Crystal Studio v$Version，正文取 CHANGELOG 该版小节（不向上游提 PR）"
    Write-Line "6. Docker 镜像 tag：v$Version / X.Y / latest（crystalstudio/miredit、crystalstudio/miredit-front）"
    Write-Line '   ⚠ 现状 compose 用 :dev，且整套编排尚未在 CI 或任何机器上实跑验证（docker/README.md）'
    Write-Line '7. 收尾复核：tag 指向的提交上 /api/health 版本 = tag 版本；CHANGELOG 无占位日期；工作树干净'
    Write-Line ''
    Write-Line "本次改动：根 Cargo.toml 的 [$WorkspaceTable].version：$OldVersion → $Version；CHANGELOG.md 新增 [ $Version ] 小节并重建空 [Unreleased]。"
    Write-Line '未做：变更条目（人写）、门禁、tag、Release、镜像 —— 这些都在上面清单里。'
}

# ───────────────────────────── 主流程 ─────────────────────────────

if ([string]::IsNullOrWhiteSpace($Root)) {
    $rootPath = Split-Path -Parent $PSScriptRoot
}
else {
    if (-not (Test-Path -LiteralPath $Root -PathType Container)) {
        Stop-Failure "指定的 -Root 不是目录：$Root"
    }
    $rootPath = (Resolve-Path -LiteralPath $Root).ProviderPath
}

$hasMode = $PSBoundParameters.ContainsKey('Check') -or $PSBoundParameters.ContainsKey('Part') -or $PSBoundParameters.ContainsKey('Set')
if (-not $hasMode) {
    Write-Line '[提示] 未指定 -Check / -Part / -Set，按 -Check 执行（只读）。'
    $Check = $true
}

if ($Check) {
    exit (Invoke-Check -RootPath $rootPath)
}

# -Part / -Set：先校验前置条件，任何一处不满足就停手，绝不半写
$pre = Invoke-Precheck -RootPath $rootPath
if ($pre.Problems.Count -gt 0) {
    Write-Line "[前置校验] 项目根：$rootPath"
    Write-Line "❌ 前置校验未通过：$($pre.Problems.Count) 处问题，未改动任何文件"
    foreach ($p in $pre.Problems) { Write-Line "   - $p" }
    foreach ($h in (Get-SingleSourceHint)) { Write-Line $h }
    Write-Line ''
    Write-Line '退出码 1'
    exit 1
}

$current = $pre.Info.RootVersion

if ($PSBoundParameters.ContainsKey('Part')) {
    $target = Get-NextVersion -Current $current -Kind $Part
    $why = "按 -Part $Part 递增"
}
else {
    if ($Set -match '[-+]') {
        Stop-Failure "-Set 只接受正式三段版本 X.Y.Z" @(
            "实得：$Set",
            '预发布 -rc.N 与构建元数据 +g<sha> 由发布流程另定（VERSIONING.md §二），本脚本不处理。'
        )
    }
    if (-not (Test-SemVerCore $Set)) {
        Stop-Failure "-Set 的值不是合法三段 SemVer：$Set" @(
            '要求：MAJOR.MINOR.PATCH，三段十进制整数，不写前导零（例：0.0.2 / 1.0.0）。'
        )
    }
    $target = $Set
    $why = '按 -Set 显式设置'
}

if ($target -eq $current) {
    Stop-Failure "目标版本与当前版本相同（$current），没有可做的改动"
}

Write-Line "[递增] 当前版本：$current（$why）"
Write-Line "[递增] 目标版本：$target"

# ① 先把两份新内容都算出来：所有校验都在内存里做完，能不写盘就不写盘
$rootManifest = $pre.Info.RootManifest
$rootOriginal = Get-FileText $rootManifest

$changelogPath = Join-Path $rootPath 'CHANGELOG.md'
if (-not (Test-Path -LiteralPath $changelogPath -PathType Leaf)) {
    Stop-Failure "找不到 CHANGELOG.md：$changelogPath（本次未改动任何文件）"
}
$changelogOriginal = Get-FileText $changelogPath

$rootNew = New-WorkspaceVersionText -Text $rootOriginal -LineNumber $pre.Info.RootLine -OldVersion $current -NewVersion $target
$changelogNew = New-ChangelogText -Text $changelogOriginal -Version $target

# ② 落盘前自检：新文本形状不对就一个字节都不写
Assert-ChangelogShape -Text $changelogNew.Text -Version $target -Context '落盘前自检' -Hint '本次未改动任何文件；这是脚本自身的缺陷。'

# ③ 落盘（两份都写，写失败则都回滚，不留半写）
Commit-Bump -RootManifest $rootManifest -RootOriginalText $rootOriginal -RootNewText $rootNew `
    -ChangelogPath $changelogPath -ChangelogOriginalText $changelogOriginal -ChangelogNewText $changelogNew.Text

Write-Line "   ✅ 根 Cargo.toml:$($pre.Info.RootLine) [$WorkspaceTable].version：$current → $target"
Write-Line "   ✅ CHANGELOG.md：'## [Unreleased]' → '## [$target] - $($changelogNew.Today)'，并在顶部重建空的 '## [Unreleased]'"

Assert-PostWrite -RootPath $rootPath -ExpectedVersion $target
Write-Line '   ✅ 写盘后复核通过：根版本可读回、CHANGELOG 有唯一带日期的版本标题与唯一 [Unreleased]'

Write-Line ''
if (-not $changelogNew.HadEntries) {
    Write-Line "❌ 注意：本次 [Unreleased] 里一条变更条目都没有 ⇒ [ $target ] 小节现在是空的。"
    Write-Line '   脚本不编造条目（VERSIONING.md §九 第 3 条）：发布前必须人工补写「新增 / 变更 / 修复 / 不兼容变更」等小节。'
}
else {
    Write-Line '⚠ 提醒：脚本只搬了标题，没有改动任何一条变更条目。'
    Write-Line "   发布前人工核对 [ $target ] 下的条目是否完整、是否要补「不兼容变更」小节（VERSIONING.md §5.2）。"
    if ($changelogNew.HasNoneNote) {
        Write-Line ('⚠ 注意：搬进 [ {0} ] 的正文里有「暂无条目」类说明（原 [Unreleased] 的备注）——' -f $target)
        Write-Line '   它未必属于本版内容，发布前确认要不要删掉或改写成真正的条目。'
    }
}
Write-Line "⚠ 版本标题的日期取系统当天（$($changelogNew.Today)）；若实际发布日不同，发布前手工改成真实日期（VERSIONING.md §九 第 2 条）。"

Show-ReleaseChecklist -Version $target -OldVersion $current
Write-Line ''
Write-Line '退出码 0'
exit 0

#Requires -Version 7.0
param(
    [Parameter(Mandatory)][string]$ActualDirectory,
    [string]$BaselineDirectory = 'automation/ui-baseline',
    [string]$OutputDirectory = 'artifacts/automation/visual',
    [switch]$UpdateBaseline
)
. "$PSScriptRoot/automation-common.ps1"
Add-Type -AssemblyName System.Drawing.Common
if (-not ('GitHubSpPixelDiff' -as [type])) {
    # .NET 10 将部分 GDI+ 类型拆到独立程序集；同时兼容 Runner 上的 .NET 8。
    $drawingReferences = @('System.Drawing.Common', 'System.Drawing.Primitives', 'System.Runtime.InteropServices') +
        @(Get-ChildItem -LiteralPath $PSHOME -Filter 'System.Private.Windows.*.dll' | Select-Object -ExpandProperty FullName)
    Add-Type -ReferencedAssemblies $drawingReferences -TypeDefinition @'
using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
public static class GitHubSpPixelDiff {
    public static long Compare(string reference, string actual, string diffFile, int tolerance) {
        using var a = new Bitmap(reference);
        using var b = new Bitmap(actual);
        if (a.Size != b.Size) return -1;
        using var diff = new Bitmap(a.Width, a.Height, PixelFormat.Format32bppArgb);
        var rect = new Rectangle(0, 0, a.Width, a.Height);
        var da = a.LockBits(rect, ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb);
        var db = b.LockBits(rect, ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb);
        var dd = diff.LockBits(rect, ImageLockMode.WriteOnly, PixelFormat.Format32bppArgb);
        long changed = 0;
        try {
            var ra = new byte[a.Width * 4]; var rb = new byte[ra.Length]; var rd = new byte[ra.Length];
            for (int y = 0; y < a.Height; y++) {
                Marshal.Copy(IntPtr.Add(da.Scan0, y * da.Stride), ra, 0, ra.Length);
                Marshal.Copy(IntPtr.Add(db.Scan0, y * db.Stride), rb, 0, rb.Length);
                for (int x = 0; x < a.Width; x++) {
                    int i = x * 4;
                    bool different = false;
                    for (int c = 0; c < 4; c++) different |= Math.Abs(ra[i+c] - rb[i+c]) > tolerance;
                    if (different) changed++;
                    rd[i] = different ? (byte)60 : rb[i]; rd[i+1] = different ? (byte)30 : rb[i+1];
                    rd[i+2] = different ? (byte)255 : rb[i+2]; rd[i+3] = 255;
                }
                Marshal.Copy(rd, 0, IntPtr.Add(dd.Scan0, y * dd.Stride), rd.Length);
            }
        } finally { a.UnlockBits(da); b.UnlockBits(db); diff.UnlockBits(dd); }
        if (changed > 0) diff.Save(diffFile, ImageFormat.Png);
        return changed;
    }
}
'@
}
$actual = Resolve-AutomationPath $ActualDirectory
$baseline = Resolve-AutomationPath $BaselineDirectory
$output = Resolve-AutomationPath $OutputDirectory
New-Item -ItemType Directory -Path $output -Force | Out-Null
$manifest = Get-Content -LiteralPath (Join-Path $baseline 'manifest.json') -Raw | ConvertFrom-Json
if (@($manifest.images).Count -eq 0 -or $manifest.channelTolerance -isnot [long] -or
    $manifest.channelTolerance -lt 0 -or $manifest.channelTolerance -gt 255 -or
    $null -eq $manifest.maxChangedRatio -or $manifest.maxChangedRatio -lt 0 -or $manifest.maxChangedRatio -gt 1) {
    throw '视觉基线清单或阈值无效。'
}
$results = foreach ($name in $manifest.images) {
    if ($name -notmatch '^[a-z0-9-]+\.png$') { throw '基线文件名无效。' }
    $current = Join-Path $actual $name
    $reference = Join-Path $baseline $name
    if (-not (Test-Path -LiteralPath $current -PathType Leaf)) { throw "缺少实际截图：$name" }
    if ($UpdateBaseline) {
        Copy-Item -LiteralPath $current -Destination $reference -Force
        continue
    }
    if (-not (Test-Path -LiteralPath $reference -PathType Leaf)) { throw "缺少视觉基线：$name。首次建立基线必须显式运行 -UpdateBaseline。" }
    $changed = [GitHubSpPixelDiff]::Compare($reference, $current, (Join-Path $output $name), $manifest.channelTolerance)
    $bitmap = [Drawing.Bitmap]::new($reference)
    try { $ratio = if ($changed -lt 0) { 1.0 } else { $changed / [double]($bitmap.Width * $bitmap.Height) } }
    finally { $bitmap.Dispose() }
    [ordered]@{ File = $name; ChangedPixels = $changed; ChangedRatio = $ratio; Passed = ($changed -ge 0 -and $ratio -le $manifest.maxChangedRatio) }
}
if ($UpdateBaseline) { Write-Host '视觉基线已显式更新；请审阅图片和差异后提交。'; return }
$passed = @($results | Where-Object { -not $_.Passed }).Count -eq 0
[ordered]@{ Passed = $passed; Images = @($results) } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $output 'visual.json') -Encoding utf8
if (-not $passed) { throw "视觉回归未通过，差异图和报告：$output" }
Write-Host "视觉回归通过：$(@($results).Count) 张图片。"

Add-Type -AssemblyName System.Drawing
Add-Type @"
using System; using System.Runtime.InteropServices;
public class FP {
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out R r);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out R r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint f);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  public struct R { public int L,T,Rt,B; }
}
"@
function Shot($p, $file) {
  $h=$p.MainWindowHandle; $r=New-Object FP+R; [FP]::GetWindowRect($h,[ref]$r)|Out-Null
  $bmp=New-Object System.Drawing.Bitmap ($r.Rt-$r.L),($r.B-$r.T); $g=[System.Drawing.Graphics]::FromImage($bmp)
  $hdc=$g.GetHdc(); [FP]::PrintWindow($h,$hdc,2)|Out-Null; $g.ReleaseHdc($hdc); $bmp.Save($file); $g.Dispose(); $bmp.Dispose()
}
function MkLP($x,$y){ [IntPtr](($y -shl 16) -bor ($x -band 0xFFFF)) }
function MMove($p,$x,$y,$btn=1){ [FP]::PostMessage($p.MainWindowHandle,0x0200,[IntPtr]$btn,(MkLP $x $y))|Out-Null; Start-Sleep -Milliseconds 30 }
function Down($p,$x,$y){ MMove $p $x $y 0; [FP]::PostMessage($p.MainWindowHandle,0x0201,[IntPtr]1,(MkLP $x $y))|Out-Null; Start-Sleep -Milliseconds 60 }
function Up($p,$x,$y){ [FP]::PostMessage($p.MainWindowHandle,0x0202,[IntPtr]0,(MkLP $x $y))|Out-Null; Start-Sleep -Milliseconds 60 }
function RDown($p,$x,$y){ MMove $p $x $y 0; [FP]::PostMessage($p.MainWindowHandle,0x0204,[IntPtr]2,(MkLP $x $y))|Out-Null; Start-Sleep -Milliseconds 60 }
function RUp($p,$x,$y){ [FP]::PostMessage($p.MainWindowHandle,0x0205,[IntPtr]0,(MkLP $x $y))|Out-Null; Start-Sleep -Milliseconds 60 }
function Drag($p,$x0,$y0,$x1,$y1,$steps=10){ Down $p $x0 $y0; for($i=1;$i -le $steps;$i++){ MMove $p ([int]($x0+($x1-$x0)*$i/$steps)) ([int]($y0+($y1-$y0)*$i/$steps)) }; Up $p $x1 $y1 }
function Key($p,$vk,$down=$true){ $m = if($down){0x0100}else{0x0101}; [FP]::PostMessage($p.MainWindowHandle,$m,[IntPtr]$vk,[IntPtr]0)|Out-Null; Start-Sleep -Milliseconds 60 }
function Tap($p,$vk){ Key $p $vk $true; Key $p $vk $false }
function Chr($p,$c){ [FP]::PostMessage($p.MainWindowHandle,0x0102,[IntPtr][int][char]$c,[IntPtr]0)|Out-Null; Start-Sleep -Milliseconds 30 }
function Start-FP($args2){ $exe="C:\Users\Tim\AppData\Local\ferrum-paint-target\debug\ferrum-paint.exe"; if($args2){ $p=Start-Process $exe -ArgumentList $args2 -PassThru -RedirectStandardError "$env:TEMP\fp_err.txt" } else { $p=Start-Process $exe -PassThru -RedirectStandardError "$env:TEMP\fp_err.txt" }; Start-Sleep 5; $p.Refresh(); return $p }


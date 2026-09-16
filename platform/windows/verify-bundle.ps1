# Reject a Windows binary that would not open a document the way a reader
# double-clicking one expects.
#
# Three things are checked, all of them read out of the image itself rather
# than inferred from how it was built, because what ships is the image:
#
#   1. The subsystem. Windows starts a console-subsystem image with a console
#      of its own whenever no terminal already has one — which is every way
#      the installer offers to start Arto: a file association, the Start menu,
#      the desktop shortcut. Only a GUI-subsystem image is free of that, so the
#      bundled binary must be one, and the single-file download built with
#      `--features windows-console` must not.
#   2. The imported DLLs. An image that imports VCRUNTIME140.dll or
#      VCRUNTIME140_1.dll needs the Visual C++ redistributable, and on a
#      machine without it the loader refuses the image before `main` — the
#      reader sees "VCRUNTIME140_1.dll was not found" and nothing else. The
#      static CRT in .cargo/config.toml is what keeps those out.
#   3. The icon, when one is named. Arto's .ico has to be linked into the
#      executable, or Explorer, the Start menu shortcut and every associated
#      document show the Dioxus CLI's own default icon instead.
#
# The PE offsets below: the DOS stub records the PE signature's offset at 0x3C;
# the 4-byte signature is followed by the 20-byte COFF header and then the
# optional header, whose magic says PE32 (0x10b) or PE32+ (0x20b). The
# subsystem sits 68 bytes into the optional header in both. The data
# directories follow the optional header's fixed part — 96 bytes for PE32, 112
# for PE32+ — and the second of them (index 1) is the import table. Section
# headers follow the optional header and are 40 bytes each.

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Path,
    # `gui` for the bundled app, `console` for the standalone download.
    [ValidateSet('gui', 'console')][string]$Expect = 'gui',
    # The .ico the image is expected to carry. Skipped when not given.
    [string]$Icon
)

$ErrorActionPreference = 'Stop'

$IMAGE_SUBSYSTEM_WINDOWS_GUI = 2
$IMAGE_SUBSYSTEM_WINDOWS_CUI = 3

if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
    throw "no binary at '$Path'"
}

$image = [System.IO.File]::ReadAllBytes($Path)

function Read-UInt16([int]$at) { [System.BitConverter]::ToUInt16($image, $at) }
function Read-UInt32([int]$at) { [System.BitConverter]::ToUInt32($image, $at) }

$peOffset = [int](Read-UInt32 0x3C)
if ($image[$peOffset] -ne 0x50 -or $image[$peOffset + 1] -ne 0x45 -or
    $image[$peOffset + 2] -ne 0 -or $image[$peOffset + 3] -ne 0) {
    throw "'$Path' is not a PE image"
}

$coff = $peOffset + 4
$sectionCount = [int](Read-UInt16 ($coff + 2))
$optionalSize = [int](Read-UInt16 ($coff + 16))
$optional = $coff + 20
$magic = Read-UInt16 $optional
$plus = $magic -eq 0x20b

# --- 1. Subsystem ---

$subsystem = Read-UInt16 ($optional + 68)
$want = if ($Expect -eq 'gui') { $IMAGE_SUBSYSTEM_WINDOWS_GUI } else { $IMAGE_SUBSYSTEM_WINDOWS_CUI }
if ($subsystem -ne $want) {
    $names = @{ $IMAGE_SUBSYSTEM_WINDOWS_GUI = 'GUI'; $IMAGE_SUBSYSTEM_WINDOWS_CUI = 'console' }
    $found = if ($names.ContainsKey([int]$subsystem)) { $names[[int]$subsystem] } else { "subsystem $subsystem" }
    throw "'$Path' is a $found binary, expected $Expect (see the comment at the top of crates/arto/src/main.rs)"
}

# --- 2. Imported DLLs ---

$sections = $optional + $optionalSize
# An address the image uses at runtime, as an offset into the file on disk.
function Resolve-Rva([uint32]$rva) {
    for ($i = 0; $i -lt $sectionCount; $i++) {
        $header = $sections + 40 * $i
        $size = Read-UInt32 ($header + 8)
        $start = Read-UInt32 ($header + 12)
        $raw = Read-UInt32 ($header + 20)
        if ($rva -ge $start -and $rva -lt $start + $size) {
            return [int]($raw + ($rva - $start))
        }
    }
    throw "'$Path': address 0x$($rva.ToString('x')) is in no section"
}

$importRva = Read-UInt32 ($optional + $(if ($plus) { 112 } else { 96 }) + 8)
$imports = @()
if ($importRva -ne 0) {
    # A null descriptor ends the table; each is 20 bytes and names its DLL
    # through the RVA 12 bytes in.
    $descriptor = Resolve-Rva $importRva
    while ((Read-UInt32 ($descriptor + 12)) -ne 0) {
        $name = Resolve-Rva (Read-UInt32 ($descriptor + 12))
        $end = $name
        while ($image[$end] -ne 0) { $end++ }
        $imports += [System.Text.Encoding]::ASCII.GetString($image, $name, $end - $name)
        $descriptor += 20
    }
}

$redistributable = $imports | Where-Object { $_ -match '^(vcruntime|msvcp)\d' }
if ($redistributable) {
    throw ("'$Path' imports $($redistributable -join ', '), which ships with the Visual C++ " +
        "redistributable rather than with Windows; see the static CRT in .cargo/config.toml")
}

# --- 3. Icon ---

if ($Icon) {
    if (-not (Test-Path -LiteralPath $Icon -PathType Leaf)) {
        throw "no icon at '$Icon'"
    }
    # The resource compiler copies each image in the .ico into a resource of
    # its own, byte for byte, so if the icon was linked in at all its images
    # are in the executable verbatim. The one with the most bytes makes the
    # least mistakable needle. A directory entry gives that length 8 bytes in
    # and the offset 12 bytes in; entries are 16 bytes each, after the 6-byte
    # header.
    $ico = [System.IO.File]::ReadAllBytes($Icon)
    $count = [System.BitConverter]::ToUInt16($ico, 4)
    $largest = 0
    $at = 0
    for ($i = 0; $i -lt $count; $i++) {
        $entry = 6 + 16 * $i
        $size = [System.BitConverter]::ToUInt32($ico, $entry + 8)
        if ($size -gt $largest) {
            $largest = $size
            $at = [System.BitConverter]::ToUInt32($ico, $entry + 12)
        }
    }

    # Latin-1 maps every byte to the character of the same value, so this is a
    # byte search wearing a string's clothes — and .NET's is fast enough to run
    # over a 30 MB image.
    $latin1 = [System.Text.Encoding]::Latin1
    $haystack = $latin1.GetString($image)
    $needle = $latin1.GetString($ico, $at, $largest)
    if ($haystack.IndexOf($needle, [System.StringComparison]::Ordinal) -lt 0) {
        throw ("'$Path' does not carry the icon from '$Icon'; the executable would show the " +
            'Dioxus CLI''s default icon instead (see the icon list in crates/arto/Dioxus.toml)')
    }
}

$carries = if ($Icon) { ', carrying its icon' } else { '' }
Write-Host "$Path is a $Expect binary with no redistributable CRT imports$carries"

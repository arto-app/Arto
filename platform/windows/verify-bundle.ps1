# Reject a Windows app binary that would open a command prompt behind the
# document it was asked to read.
#
# Windows starts a console-subsystem image with a console of its own whenever
# no terminal already has one — which is every way the installer offers to
# start Arto: a file association, the Start menu, the desktop shortcut. Only a
# GUI-subsystem image is free of that, so the bundled binary must be one, and
# the single-file download built with `--features windows-console` must not.
#
# The subsystem is a field of the PE optional header, so it is read straight
# out of the image rather than inferred from how it was built. The field sits
# 68 bytes into that header for both PE32 and PE32+, and the header follows the
# 4-byte PE signature and the 20-byte COFF header at the offset the DOS stub
# records at 0x3C.

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Path,
    # `gui` for the bundled app, `console` for the standalone download.
    [ValidateSet('gui', 'console')][string]$Expect = 'gui'
)

$ErrorActionPreference = 'Stop'

$IMAGE_SUBSYSTEM_WINDOWS_GUI = 2
$IMAGE_SUBSYSTEM_WINDOWS_CUI = 3

if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
    throw "no binary at '$Path'"
}

$stream = [System.IO.File]::OpenRead($Path)
try {
    $reader = [System.IO.BinaryReader]::new($stream)
    $stream.Position = 0x3C
    $peOffset = $reader.ReadInt32()
    $stream.Position = $peOffset
    $signature = $reader.ReadBytes(4)
    if ($signature[0] -ne 0x50 -or $signature[1] -ne 0x45 -or $signature[2] -ne 0 -or $signature[3] -ne 0) {
        throw "'$Path' is not a PE image"
    }
    $stream.Position = $peOffset + 4 + 20 + 68
    $subsystem = $reader.ReadUInt16()
}
finally {
    $stream.Dispose()
}

$want = if ($Expect -eq 'gui') { $IMAGE_SUBSYSTEM_WINDOWS_GUI } else { $IMAGE_SUBSYSTEM_WINDOWS_CUI }
if ($subsystem -ne $want) {
    $names = @{ $IMAGE_SUBSYSTEM_WINDOWS_GUI = 'GUI'; $IMAGE_SUBSYSTEM_WINDOWS_CUI = 'console' }
    $found = if ($names.ContainsKey([int]$subsystem)) { $names[[int]$subsystem] } else { "subsystem $subsystem" }
    throw "'$Path' is a $found binary, expected $Expect (see the comment at the top of crates/arto/src/main.rs)"
}

Write-Host "$Path is a $Expect binary"

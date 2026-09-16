"""Build Arto.ico from a square RGBA PNG, with the standard library alone.

    python3 platform/windows/make-icon.py crates/arto/assets/Arto.png \
        platform/windows/Arto.ico

The result is committed rather than built, the way the macOS .icns is: it
changes only when the artwork does, and no toolchain on any build machine has
to be able to produce it.

Entries up to 128px are written as 32-bit BMP, which every resource compiler
and NSIS has always accepted; the 256px entry is written as PNG, which is how
the format carries that size. That is the shape `tauri icon` and the `ico`
crate produce, so the file travels through rc.exe and makensis unremarkably.

Stdlib only — no Pillow, no ImageMagick — because the one machine that has to
run this is whichever one the artwork was last edited on.
"""

import struct
import sys
import zlib

SIZES = [16, 24, 32, 48, 64, 128, 256]


def read_png(path):
    """Decode a non-interlaced 8-bit RGBA PNG into (width, height, rows)."""
    data = open(path, "rb").read()
    assert data[:8] == b"\x89PNG\r\n\x1a\n", "not a PNG"
    pos = 8
    width = height = None
    idat = bytearray()
    while pos < len(data):
        (length,) = struct.unpack(">I", data[pos : pos + 4])
        kind = data[pos + 4 : pos + 8]
        body = data[pos + 8 : pos + 8 + length]
        pos += 12 + length
        if kind == b"IHDR":
            width, height, depth, color, compression, filt, interlace = struct.unpack(
                ">IIBBBBB", body
            )
            assert depth == 8 and color == 6, f"want 8-bit RGBA, got depth {depth} colour {color}"
            assert interlace == 0, "interlaced PNG unsupported"
        elif kind == b"IDAT":
            idat += body
        elif kind == b"IEND":
            break

    raw = zlib.decompress(bytes(idat))
    stride = width * 4
    rows = []
    previous = bytearray(stride)
    pos = 0
    for _ in range(height):
        filter_type = raw[pos]
        pos += 1
        line = bytearray(raw[pos : pos + stride])
        pos += stride
        for i in range(stride):
            a = line[i - 4] if i >= 4 else 0
            b = previous[i]
            c = previous[i - 4] if i >= 4 else 0
            if filter_type == 0:
                pass
            elif filter_type == 1:
                line[i] = (line[i] + a) & 0xFF
            elif filter_type == 2:
                line[i] = (line[i] + b) & 0xFF
            elif filter_type == 3:
                line[i] = (line[i] + (a + b) // 2) & 0xFF
            elif filter_type == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                pred = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
                line[i] = (line[i] + pred) & 0xFF
            else:
                raise AssertionError(f"unknown filter {filter_type}")
        rows.append(bytes(line))
        previous = line
    return width, height, rows


def resize(rows, width, height, size):
    """Area-average downscale to size×size, which is what keeps edges smooth."""
    out = []
    for y in range(size):
        y0, y1 = y * height // size, max((y + 1) * height // size, y * height // size + 1)
        line = bytearray()
        for x in range(size):
            x0, x1 = x * width // size, max((x + 1) * width // size, x * width // size + 1)
            r = g = b = a = n = 0
            for sy in range(y0, y1):
                row = rows[sy]
                for sx in range(x0, x1):
                    o = sx * 4
                    alpha = row[o + 3]
                    # Average in premultiplied space so a transparent pixel's
                    # colour cannot bleed into the edge.
                    r += row[o] * alpha
                    g += row[o + 1] * alpha
                    b += row[o + 2] * alpha
                    a += alpha
                    n += 1
            if a:
                line += bytes([r // a, g // a, b // a, a // n])
            else:
                line += bytes([0, 0, 0, 0])
        out.append(bytes(line))
    return out


def bmp_entry(rows, size):
    """A 32-bit bottom-up DIB with the (unused, all-zero) AND mask appended."""
    header = struct.pack(
        "<IiiHHIIiiII", 40, size, size * 2, 1, 32, 0, size * size * 4, 0, 0, 0, 0
    )
    pixels = bytearray()
    for row in reversed(rows):
        for x in range(size):
            o = x * 4
            pixels += bytes([row[o + 2], row[o + 1], row[o], row[o + 3]])
    mask_stride = ((size + 31) // 32) * 4
    return header + bytes(pixels) + bytes(mask_stride * size)


def png_entry(rows, size):
    def chunk(kind, body):
        return (
            struct.pack(">I", len(body))
            + kind
            + body
            + struct.pack(">I", zlib.crc32(kind + body) & 0xFFFFFFFF)
        )

    raw = bytearray()
    for row in rows:
        raw += b"\x00" + row
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
        + chunk(b"IEND", b"")
    )


def main(source, destination):
    width, height, rows = read_png(source)
    assert width == height, "want a square source"

    images = []
    for size in SIZES:
        scaled = resize(rows, width, height, size)
        images.append((size, png_entry(scaled, size) if size == 256 else bmp_entry(scaled, size)))

    out = bytearray(struct.pack("<HHH", 0, 1, len(images)))
    offset = 6 + 16 * len(images)
    for size, blob in images:
        out += struct.pack(
            "<BBBBHHII", size % 256, size % 256, 0, 0, 1, 32, len(blob), offset
        )
        offset += len(blob)
    for _, blob in images:
        out += blob

    open(destination, "wb").write(bytes(out))
    print(f"{destination}: {len(out)} bytes, {len(images)} images {SIZES}")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])

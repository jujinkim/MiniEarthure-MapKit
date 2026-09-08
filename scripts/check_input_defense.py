#!/usr/bin/env python3
"""Independent synthetic hostile ZIP/asset corpus through the real public CLI."""
import argparse
import copy
import hashlib
import io
import json
from pathlib import Path
import struct
import subprocess
import tempfile
import zipfile
import zlib

ROOT = Path(__file__).resolve().parents[1]


def canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':')).encode()


def sha(data):
    return hashlib.sha256(data).hexdigest()


def asset_package(base, payload):
    """Invalid PNG body, with independently correct inventory and world hashes."""
    with zipfile.ZipFile(io.BytesIO(base)) as archive:
        doc = json.loads(archive.read('document.json'))
        manifest = json.loads(archive.read('manifest.json'))
    doc['assets'] = [{'id': 'hostile-synthetic', 'path': 'assets/invalid.png',
                      'attribution': {'source': 'original synthetic', 'license': 'MIT', 'notice': ''}, 'collision': []}]
    manifest['assets'] = doc['assets']
    files = {'document.json': canonical(doc), 'assets/invalid.png': payload}
    manifest['files'] = [{'path': p, 'size': len(b), 'sha256': sha(b)} for p, b in sorted(files.items())]
    gameplay = copy.deepcopy(doc)
    del gameplay['provenance'], gameplay['attributions']
    manifest['world_content_hash'] = sha(canonical([gameplay, {'assets/invalid.png': sha(payload)}]))
    return envelope([('manifest.json', canonical(manifest)), *sorted(files.items())])[0]


def envelope(entries, descriptor=False, signed=True, wide=False, end64=False):
    """Small independent APPNOTE writer; offsets are returned for targeted corruption."""
    local, central, records = bytearray(), bytearray(), []
    for name, raw in entries:
        name = name.encode()
        compressor = zlib.compressobj(9, zlib.DEFLATED, -15)
        data = compressor.compress(raw) + compressor.flush()
        crc, size, expanded = zlib.crc32(raw), len(data), len(raw)
        offset = len(local)
        extra = struct.pack('<HHQQ', 1, 16, 0 if descriptor else expanded, 0 if descriptor else size) if wide else b''
        local.extend(struct.pack('<IHHHHHIIIHH', 0x04034b50, 45 if wide else 20, 8 if descriptor else 0, 8, 0, 33,
                                 0 if descriptor else crc, 0xffffffff if wide else (0 if descriptor else size),
                                 0xffffffff if wide else (0 if descriptor else expanded), len(name), len(extra)))
        local.extend(name + extra + data)
        desc = len(local)
        if descriptor:
            if signed:
                local.extend(b'PK\x07\x08')
            local.extend(struct.pack('<IQQ' if wide else '<III', crc, size, expanded))
        central.extend(struct.pack('<IHHHHHHIIIHHHHHII', 0x02014b50, 20, 45 if wide else 20, 8 if descriptor else 0,
                                   8, 0, 33, crc, size, expanded, len(name), 0, 0, 0, 0, 0, offset))
        central.extend(name)
        records.append({'local': offset, 'extra': offset + 30 + len(name), 'descriptor': desc})
    start, length = len(local), len(central)
    out = local + central
    if end64:
        z = len(out)
        out.extend(struct.pack('<IQHHIIQQQQ', 0x06064b50, 44, 45, 45, 0, 0, len(entries), len(entries), length, start))
        out.extend(struct.pack('<IIQI', 0x07064b50, 0, z, 1))
    out.extend(struct.pack('<IHHHHIIH', 0x06054b50, 0, 0, 65535 if end64 else len(entries),
                           65535 if end64 else len(entries), 0xffffffff if end64 else length,
                           0xffffffff if end64 else start, 0))
    return out, records


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--fixture-dir', type=Path, help='New output directory for native/consumer fixtures')
    args = parser.parse_args()
    cli = ROOT / 'target/debug' / ('mapkit.exe' if __import__('sys').platform == 'win32' else 'mapkit')
    count = 0
    with tempfile.TemporaryDirectory(prefix='mapkit-defense-') as temporary:
        work = Path(temporary)
        subprocess.run([str(cli), 'pack', str(ROOT / 'examples/minimal'), str(work / 'valid.memap')], check=True, capture_output=True)
        base = (work / 'valid.memap').read_bytes()
        with zipfile.ZipFile(io.BytesIO(base)) as archive:
            entries = [(name, archive.read(name)) for name in archive.namelist()]

        def check(data, expected=None):
            nonlocal count
            count += 1
            path = work / f'case-{count}.memap'
            path.write_bytes(data)
            result = subprocess.run([str(cli), 'validate', str(path)], capture_output=True, text=True, timeout=30)
            if expected:
                assert result.returncode == 1 and not result.stdout, (count, result)
                assert json.loads(result.stderr)['code'] == expected, (count, result.stderr)
            else:
                assert result.returncode == 0, (count, result.stderr)

        for descriptor in (False, True):
            for signed in ((False, True) if descriptor else (True,)):
                for wide in (False, True):
                    for end64 in (False, True):
                        data, records = envelope(entries, descriptor, signed, wide, end64)
                        check(data)
                        if descriptor:
                            for field in (0, 4, 12 if wide else 8):
                                damaged = data.copy()
                                damaged[records[0]['descriptor'] + (4 if signed else 0) + field] ^= 1
                                check(damaged, 'E_ZIP')
                        if wide:
                            damaged = data.copy()
                            damaged[records[0]['extra'] + 4] ^= 1
                            check(damaged, 'E_ZIP')
        # Full ZIP64 central sizes/offsets, not only local force_zip64 fields.
        data, _ = envelope(entries, wide=True)
        end = len(data) - 22
        central_start = struct.unpack_from('<I', data, end + 16)[0]
        central, p = bytearray(), central_start
        for _ in entries:
            header = bytearray(data[p:p+46])
            name_length = struct.unpack_from('<H', header, 28)[0]
            compressed, expanded = struct.unpack_from('<II', header, 20)
            offset = struct.unpack_from('<I', header, 42)[0]
            extra = struct.pack('<HHQQQ', 1, 24, expanded, compressed, offset)
            struct.pack_into('<II', header, 20, 0xffffffff, 0xffffffff)
            struct.pack_into('<I', header, 42, 0xffffffff)
            struct.pack_into('<H', header, 30, len(extra))
            central.extend(header + data[p+46:p+46+name_length] + extra)
            p += 46 + name_length
        footer = bytearray(data[end:])
        struct.pack_into('<I', footer, 12, len(central))
        check(data[:central_start] + central + footer)
        # Header-only PNG with an honest manifest; non-inflating inspection is insufficient.
        ihdr = struct.pack('>IIBBBBB', 2, 2, 8, 6, 0, 0, 0)
        payload = b'\x89PNG\r\n\x1a\n' + struct.pack('>I', 13) + b'IHDR' + ihdr + struct.pack('>I', zlib.crc32(b'IHDR' + ihdr))
        invalid = asset_package(base, payload)
        check(invalid, 'E_ASSET')
        if args.fixture_dir:
            args.fixture_dir.mkdir(parents=True, exist_ok=False)
            (args.fixture_dir / 'valid.memap').write_bytes(base)
            (args.fixture_dir / 'invalid-asset.memap').write_bytes(invalid)
        print(json.dumps({'status': 'PASS', 'cli_cases': count, 'valid_sha256': sha(base), 'invalid_asset_sha256': sha(invalid)}))


if __name__ == '__main__':
    main()

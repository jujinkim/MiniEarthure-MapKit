#!/usr/bin/env python3
"""Public stdlib-only CLI reproducibility audit using disposable synthetic inputs."""
import argparse
import copy
import hashlib
import io
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import zipfile
import zlib

ROOT = Path(__file__).resolve().parents[1]


def canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':')).encode('utf-8')


def digest(data):
    return hashlib.sha256(data).hexdigest()


def png(value=0):
    def chunk(kind, data):
        return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))
    samples = (b'\0' + struct.pack('>H', value) * 3) * 3
    return (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', 3, 3, 16, 0, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress(samples)) + chunk(b'IEND', b''))


def fixture():
    doc = json.loads((ROOT / 'examples/minimal/document.json').read_text(encoding='utf-8'))
    doc.update(bounds={'min': [0, 0], 'max': [800, 800]}, cell_size_cm=400, seed=2**53 - 1)
    doc['attributions'].append({'source': '한글 é e\u0301', 'license': 'MIT', 'notice': 'line\n\t"quoted" \\ /'})
    doc['nodes'] = [{'id': f'n{i}', 'position': [x, 20, y], 'level': 0}
                    for i, (x, y) in enumerate([(0, 200), (800, 200), (0, 600), (800, 600)])]
    doc['roads'] = [{'id': f'r{i}', 'from': f'n{i*2}', 'to': f'n{i*2+1}',
                     'points': [doc['nodes'][i*2]['position'], doc['nodes'][i*2+1]['position']],
                     'widths_cm': [40], 'surfaces': ['asphalt'], 'kind': 'ground'} for i in range(2)]
    doc['buildings'] = [{'id': f'b{i}', 'footprint': [[x, 300], [x+40, 300], [x+40, 340], [x, 340]],
                         'base_cm': 0, 'height_cm': 100, 'usage': 'test', 'material': 'concrete', 'roof': 'flat'}
                        for i, x in enumerate([100, 500])]
    doc['zones'] = [{'id': f'z{i}', 'polygon': [[x, 0], [x+200, 0], [x+200, 150], [x, 150]],
                     'kind': 'orchard', 'spacing_cm': 200, 'density_per_mille': 1000, 'exclusions': []}
                    for i, x in enumerate([0, 400])]
    doc['heightmaps'] = [{'cell': {'x': x, 'y': y}, 'path': f'terrain/{x}-{y}.png',
                          'spacing_cm': 200, 'offset_cm': 0, 'step_cm': 1}
                         for x in range(2) for y in range(2)]
    doc['assets'] = [{'id': f'a{i}', 'path': path, 'attribution': doc['attributions'][0],
                      'collision': [{'center': [0, 10, 0], 'size_cm': [20, 20, 20]}]}
                     for i, path in enumerate(['assets/Z.png', 'assets/a.png', 'assets/a.png'])]
    doc['placements'] = [{'id': f'p{i}', 'asset_id': f'a{i}', 'position': [50+i*400, 0, 400], 'quarter_turns': i}
                         for i in range(2)]
    return doc


def inspect_export(path, inspection):
    data = path.read_bytes()
    assert inspection['package_sha256'] == digest(data) and inspection['package_bytes'] == len(data)
    with zipfile.ZipFile(path) as archive:
        names = archive.namelist()
        assert names == ['manifest.json', *sorted(names[1:])] and not archive.comment
        position = 0
        for entry in archive.infolist():
            assert entry.header_offset == position  # physical order, no prefix or gaps
            assert entry.date_time == (1980, 1, 1, 0, 0, 0) and entry.compress_type == zipfile.ZIP_DEFLATED
            assert entry.create_system == 3 and entry.external_attr >> 16 == 0o100644
            assert not entry.extra and not entry.comment and entry.flag_bits == 0
            name_size, extra_size = struct.unpack_from('<HH', data, position + 26)
            assert data[position:position+4] == b'PK\x03\x04'
            assert data[position+30:position+30+name_size] == entry.filename.encode('ascii')
            position += 30 + name_size + extra_size + entry.compress_size
        assert data[position:position+4] == b'PK\x01\x02'
        m = json.loads(archive.read('manifest.json'))
        d = json.loads(archive.read('document.json'))
        for name, value in [('manifest.json', m), ('document.json', d)]:
            assert archive.read(name) == canonical(value), name
        assert [f['path'] for f in m['files']] == names[1:]
        for record in m['files']:
            payload = archive.read(record['path'])
            assert record['size'] == len(payload) and record['sha256'] == digest(payload)
        expanded = sum(e.file_size for e in archive.infolist())
        user_bytes = sum(len(archive.read(p)) for p in {a['path'] for a in d['assets']})
        assert inspection['expanded_bytes'] == expanded
        assert inspection['user_asset_bytes'] == user_bytes
        assert inspection['base_data_bytes'] == expanded - user_bytes
        return {name: archive.read(name) for name in names}


class Streaming(io.BytesIO):
    def seek(self, *args):
        raise io.UnsupportedOperation('nonseekable producer')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', type=Path, default=ROOT / 'target/debug/mapkit')
    args = parser.parse_args()
    calls = 0
    with tempfile.TemporaryDirectory(prefix='mapkit-reproducibility-') as directory:
        work = Path(directory)
        project = work / 'source'
        project.mkdir()
        doc = fixture()
        source = project / 'document.json'
        def save(document):
            source.write_text(json.dumps(document, ensure_ascii=False, indent=2), encoding='utf-8')
        save(doc)
        for record in doc['heightmaps'] + doc['assets']:
            path = project / record['path']
            path.parent.mkdir(exist_ok=True)
            path.write_bytes(png())
        (project / 'editor-history.json').write_text('not exported')
        def run(*arguments, env=None):
            nonlocal calls
            result = subprocess.run([str(args.cli.resolve()), *map(str, arguments)], capture_output=True,
                                    text=True, timeout=60, env=env)
            calls += 1
            assert result.returncode == 0 and not result.stderr, (arguments, result)
            return result.stdout.strip()
        def pack(name):
            output = work / f'{name}.memap'
            identity = json.loads(run('pack', project, output))
            inspect_export(output, identity)
            return output, identity
        first, identity = pack('first')
        entries = inspect_export(first, identity)
        original_source = source.read_bytes()
        normalized = json.loads(entries['document.json'])
        for field in ('nodes', 'roads', 'buildings', 'zones', 'assets', 'placements', 'heightmaps', 'attributions'):
            doc[field].reverse()
        save(doc)
        for path in project.rglob('*'):
            if path.is_file():
                os.utime(path, (946684800, 946684800))
                path.chmod(0o600)
        environment = dict(os.environ, TZ='Pacific/Honolulu', LANG='C', LC_ALL='C')
        second = work / 'second.memap'
        run('pack', project, second, env=environment)
        assert first.read_bytes() == second.read_bytes()
        for i in range(2):
            foreign = work / f'python-{i}.memap'
            result = subprocess.run([sys.executable, str(ROOT / 'examples/third_party.py'), str(foreign), str(source)],
                                    capture_output=True, text=True, check=True, timeout=30)
            assert json.loads(result.stdout)['world_content_hash'] == identity['world_content_hash']
            assert json.loads(run('validate', foreign))['world_content_hash'] == identity['world_content_hash']
            with zipfile.ZipFile(foreign) as archive:
                assert {n: archive.read(n) for n in archive.namelist()} == entries
        assert (work / 'python-0.memap').read_bytes() == (work / 'python-1.memap').read_bytes()
        run('unpack', first, work / 'unpacked')
        run('pack', work / 'unpacked', work / 'repacked.memap')
        assert first.read_bytes() == (work / 'repacked.memap').read_bytes()

        # Accept legal foreign container choices, normalize on explicit re-export.
        for variant in ('stored', 'streaming', 'zip64'):
            buffer = Streaming() if variant == 'streaming' else io.BytesIO()
            with zipfile.ZipFile(buffer, 'w', compression=zipfile.ZIP_DEFLATED) as archive:
                archive.comment = b'foreign container metadata'
                for name in ['manifest.json', *reversed(list(entries)[1:])]:
                    info = zipfile.ZipInfo(name, (2025, 4, 3, 2, 1, 0))
                    info.compress_type = zipfile.ZIP_STORED if variant == 'stored' else zipfile.ZIP_DEFLATED
                    with archive.open(info, 'w', force_zip64=variant == 'zip64') as output:
                        output.write(entries[name])
            foreign = work / f'{variant}.memap'
            foreign.write_bytes(buffer.getvalue())
            assert json.loads(run('validate', foreign))['world_content_hash'] == identity['world_content_hash']
            run('unpack', foreign, work / variant)
            run('pack', work / variant, work / f'{variant}-normalized.memap')
            assert (work / f'{variant}-normalized.memap').read_bytes() == first.read_bytes()

        def generated(path, name):
            return [run('generate-chunk', path, x, y, work / f'{name}-{x}-{y}.json')
                    for y in range(2) for x in range(2)]
        baseline = generated(first, 'base')
        metadata = copy.deepcopy(normalized)
        metadata['provenance'].update(tool_id='unknown 도구', version='other', build_id='other',
                                      fingerprint='anything', first_created='2020-01-01T00:00:00Z',
                                      last_edited='2026-09-09T09:00:00+09:00')
        metadata['attributions'][0]['notice'] += '\nExplicit edit'
        save(metadata)
        edited, edited_identity = pack('metadata')
        assert edited_identity['package_sha256'] != identity['package_sha256']
        assert edited_identity['world_content_hash'] == identity['world_content_hash']
        assert generated(edited, 'metadata') == baseline
        revision = copy.deepcopy(normalized)
        revision['revision'] += 1
        save(revision)
        assert pack('revision')[1]['world_content_hash'] != identity['world_content_hash']
        geometry = copy.deepcopy(normalized)
        geometry['roads'][0]['widths_cm'][0] += 20
        save(geometry)
        changed, changed_identity = pack('geometry')
        assert changed_identity['world_content_hash'] != identity['world_content_hash']
        assert generated(changed, 'geometry') != baseline
        save(normalized)
        (project / 'assets/a.png').write_bytes(png(1))
        assert pack('asset-bytes')[1]['world_content_hash'] != identity['world_content_hash']
        (project / 'assets/a.png').write_bytes(png())
        for heightmap in normalized['heightmaps']:
            (project / heightmap['path']).write_bytes(png(1))
        terrain, terrain_identity = pack('terrain-bytes')
        assert terrain_identity['world_content_hash'] != identity['world_content_hash']
        assert generated(terrain, 'terrain') != baseline
        assert (work / 'unpacked/document.json').read_bytes() == entries['document.json']
        assert original_source != entries['document.json']  # explicit source normalization
    print(f'Reproducibility: PASS ({calls} CLI calls; multi-payload inventory, JSON/ZIP order, '
          'permissions/time/locale, Python producer, stored/streaming/ZIP64 roundtrips, four-cell hashes)')


if __name__ == '__main__':
    main()

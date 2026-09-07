#!/usr/bin/env python3
"""Minimal independent producer: Python stdlib only, no MapKit imports or binary."""
import hashlib
import json
from pathlib import Path
import sys
import zipfile


def canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()


def digest(data):
    return hashlib.sha256(data).hexdigest()


def produce(output):
    document = json.loads((Path(__file__).parent / 'minimal/document.json').read_text())
    for field in ('nodes', 'roads', 'buildings', 'zones', 'assets', 'placements'):
        document[field].sort(key=lambda obj: obj['id'])
    document['heightmaps'].sort(key=lambda h: (h['cell']['x'], h['cell']['y']))
    document['attributions'].sort(key=lambda a: (a['source'], a['license'], a['notice']))
    payload = canonical(document)
    gameplay = {k: v for k, v in document.items() if k not in ('provenance', 'attributions')}
    manifest = {k: document[k] for k in ('map_id', 'revision', 'bounds', 'cell_size_cm', 'seed', 'theme', 'assets', 'attributions', 'provenance')}
    manifest.update(format='memap', format_version=1, recipe_version=1, generated_version=6,
                    document='document.json', files=[{'path': 'document.json', 'size': len(payload), 'sha256': digest(payload)}],
                    world_content_hash=digest(canonical([gameplay, {}])))
    with zipfile.ZipFile(output, 'x') as archive:
        for name, data in [('manifest.json', canonical(manifest)), ('document.json', payload)]:
            info = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.create_system = 3
            info.external_attr = 0o100644 << 16
            archive.writestr(info, data, compresslevel=9)
    print(json.dumps({'package_sha256': digest(Path(output).read_bytes()), 'world_content_hash': manifest['world_content_hash']}))


if __name__ == '__main__':
    produce(sys.argv[1])

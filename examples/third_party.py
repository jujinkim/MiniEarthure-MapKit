#!/usr/bin/env python3
"""Minimal independent producer: Python stdlib only, no MapKit imports or binary."""
import hashlib
import json
from pathlib import Path
import re
import sys
import zipfile


def canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()


def digest(data):
    return hashlib.sha256(data).hexdigest()


def produce(output, document_path=None):
    source = Path(document_path or Path(__file__).parent / 'minimal/document.json').resolve()
    document = json.loads(source.read_text(encoding='utf-8'))
    for field in ('nodes', 'roads', 'buildings', 'zones', 'assets', 'placements'):
        document[field].sort(key=lambda obj: obj['id'])
    document['heightmaps'].sort(key=lambda h: (h['cell']['x'], h['cell']['y']))
    document['attributions'].sort(key=lambda a: (a['source'], a['license'], a['notice']))
    # Omitted optional fields have the same canonical typed form as explicit null.
    for road in document['roads']:
        for field in ('clearance_cm', 'sidewalk_cm'):
            road.setdefault(field, None)
    for heightmap in document['heightmaps']:
        heightmap.setdefault('source_accuracy_cm', None)
    files = {'document.json': canonical(document)}
    paths = {record['path'] for record in document['heightmaps'] + document['assets']}
    for path in sorted(paths):
        parts = path.split('/')
        if (path in ('manifest.json', 'document.json') or len(path) > 240
                or any(not re.fullmatch(r'[A-Za-z0-9_ .-]+', part) or part in ('.', '..')
                       or part.endswith(('.', ' ')) for part in parts)):
            raise ValueError('unsafe or reserved payload path')
        actual = (source.parent / path).resolve()
        if not actual.is_relative_to(source.parent):
            raise ValueError('payload escapes document directory')
        files[path] = actual.read_bytes()
    files = dict(sorted(files.items()))
    gameplay = {k: v for k, v in document.items() if k not in ('provenance', 'attributions')}
    manifest = {k: document[k] for k in ('map_id', 'revision', 'bounds', 'cell_size_cm', 'seed', 'theme', 'assets', 'attributions', 'provenance')}
    manifest.update(format='memap', format_version=1, recipe_version=document["recipe_version"], generated_version=6,
                    document='document.json', files=[{'path': path, 'size': len(data), 'sha256': digest(data)} for path, data in files.items()],
                    world_content_hash=digest(canonical([gameplay, {path: digest(data) for path, data in files.items() if path != 'document.json'}])))
    with zipfile.ZipFile(output, 'x') as archive:
        for name, data in [('manifest.json', canonical(manifest)), *files.items()]:
            info = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.create_system = 3
            info.external_attr = 0o100644 << 16
            archive.writestr(info, data, compresslevel=9)
    print(json.dumps({'package_sha256': digest(Path(output).read_bytes()), 'world_content_hash': manifest['world_content_hash']}))


if __name__ == '__main__':
    produce(*sys.argv[1:])

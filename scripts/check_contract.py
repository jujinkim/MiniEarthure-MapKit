#!/usr/bin/env python3
"""Public CLI + independent producer + Draft 7 Schema acceptance regressions.

Requires Python jsonschema (only this development check, never package production).
Uses new temporary outputs and original synthetic data; no engine/private repository.
"""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import zipfile

from jsonschema import Draft7Validator

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', type=Path, default=ROOT / 'target/debug/mapkit')
    args = parser.parse_args()
    calls = 0

    def run(*arguments, error=None):
        nonlocal calls
        result = subprocess.run([str(args.cli.resolve()), *map(str, arguments)], capture_output=True, text=True, timeout=60)
        calls += 1
        if error:
            assert result.returncode == 1 and not result.stdout, (arguments, result)
            failure = json.loads(result.stderr)
            assert set(failure) == {'code', 'message'} and failure['code'] == error and failure['message'], (arguments, failure)
        else:
            assert result.returncode == 0 and not result.stderr, (arguments, result)
        return result.stdout.strip()

    schemas = {}
    for kind in ('document', 'manifest'):
        schema = json.loads((ROOT / 'spec' / f'{kind}.schema.json').read_text())
        Draft7Validator.check_schema(schema)
        schemas[kind] = Draft7Validator(schema)
    document = json.loads((ROOT / 'examples/minimal/document.json').read_text())

    with tempfile.TemporaryDirectory(prefix='mapkit-contract-') as directory:
        work = Path(directory)

        def produce(name, source):
            path = work / f'{name}.json'
            path.write_text(json.dumps(source, ensure_ascii=False))
            output = work / f'{name}.memap'
            result = subprocess.run([sys.executable, str(ROOT / 'examples/third_party.py'), str(output), str(path)], capture_output=True, text=True, timeout=30, check=True)
            return output, json.loads(result.stdout)

        package, identity = produce('independent', document)
        schemas['document'].validate(document)
        largest_seed = copy.deepcopy(document)
        largest_seed['seed'] = 2**53 - 1
        schemas['document'].validate(largest_seed)
        largest_seed['seed'] += 1
        assert not schemas['document'].is_valid(largest_seed)
        inspected = json.loads(run('inspect', package))
        validated = json.loads(run('validate', package))
        assert inspected.pop('manifest')['provenance'] == document['provenance']
        assert inspected == validated
        assert all(validated[key] == value for key, value in identity.items())
        with zipfile.ZipFile(package) as archive:
            manifest = json.loads(archive.read('manifest.json'))
            schemas['manifest'].validate(manifest)
        run('unpack', package, work / 'project')
        repacked = json.loads(run('pack', work / 'project', work / 'repacked.memap'))
        assert repacked['world_content_hash'] == identity['world_content_hash']
        chunk_path = work / 'chunk.json'
        generated_hash = run('generate-chunk', package, 0, 0, chunk_path)
        assert generated_hash == hashlib.sha256(chunk_path.read_bytes()).hexdigest()
        assert json.loads(chunk_path.read_bytes())['format_version'] == 6
        for kind in schemas:
            destination = work / f'{kind}.schema.json'
            run('schema', kind, destination)
            assert json.loads(destination.read_bytes()) == schemas[kind].schema

        changed = copy.deepcopy(document)
        changed['provenance'].update(tool_id='Unknown producer 도구', version='next', build_id='external', fingerprint='not-a-signature', last_edited='2026-09-08T09:00:00+09:00')
        changed['attributions'][0]['notice'] = 'Preserved original license\nplus edit notice'
        changed_path = work / 'project/document.json'
        changed_path.write_text(json.dumps(changed, ensure_ascii=False))
        edited = json.loads(run('pack', work / 'project', work / 'edited.memap'))
        assert edited['world_content_hash'] == identity['world_content_hash']
        assert edited['package_sha256'] != repacked['package_sha256']
        assert run('generate-chunk', work / 'edited.memap', 0, 0, work / 'edited-chunk.json') == generated_hash
        run('unpack', work / 'edited.memap', work / 'edited-project')
        assert json.loads((work / 'edited-project/document.json').read_bytes())['provenance'] == changed['provenance']

        # Externally produced, fully rehashed bad metadata must fail every reader,
        # not just the MapKit exporter or a stale inventory hash.
        invalid = []
        for field in ('tool_id', 'version', 'build_id', 'fingerprint'):
            invalid.append((('provenance', field), ' ', 'E_PROVENANCE', True))
        invalid += [
            (('provenance', 'first_created'), 'tomorrow', 'E_PROVENANCE', True),
            (('provenance', 'last_edited'), '2026-02-30T00:00:00Z', 'E_PROVENANCE', False),
            (('provenance', 'last_edited'), '2026-09-06T00:00:00Z', 'E_PROVENANCE', False),
            (('attributions', 0, 'source'), '', 'E_ATTRIBUTION', True),
            (('attributions', 0, 'license'), ' ', 'E_ATTRIBUTION', True),
            (('provenance', 'future'), 'unknown', 'E_JSON', True),
        ]
        for index, (path, value, code, schema_rejects) in enumerate(invalid):
            bad = copy.deepcopy(document)
            target = bad
            for key in path[:-1]:
                target = target[key]
            target[path[-1]] = value
            if schema_rejects:
                assert not schemas['document'].is_valid(bad), path
            foreign, _ = produce(f'bad-{index}', bad)
            with zipfile.ZipFile(foreign) as archive:
                if schema_rejects:
                    assert not schemas['manifest'].is_valid(json.loads(archive.read('manifest.json'))), path
            for command in ('inspect', 'validate'):
                run(command, foreign, error=code)
            run('unpack', foreign, work / f'bad-unpack-{index}', error=code)
            run('generate-chunk', foreign, 0, 0, work / f'bad-chunk-{index}.json', error=code)
            assert not (work / f'bad-unpack-{index}').exists()
            assert not (work / f'bad-chunk-{index}.json').exists()
            (work / 'project/document.json').write_text(json.dumps(bad))
            run('pack', work / 'project', work / f'bad-pack-{index}.memap', error=code)
            assert not (work / f'bad-pack-{index}.memap').exists()

        # Existing caller outputs remain untouched on errors, including usage.
        original = package.read_bytes()
        (work / 'project/document.json').write_text(json.dumps(document))
        run('pack', work / 'project', package, error='E_IO')
        run('unpack', package, work / 'project', error='E_IO')
        run('generate-chunk', package, 0, 0, chunk_path, error='E_IO')
        assert package.read_bytes() == original and hashlib.sha256(chunk_path.read_bytes()).hexdigest() == generated_hash
        run(error='E_USAGE')
        run('inspect', error='E_USAGE')
        run('inspect', package, 'extra', error='E_USAGE')
        run('generate-chunk', package, 'x', 0, work / 'unused', error='E_USAGE')
        run('generate-chunk', package, -1, 0, work / 'unused', error='E_CELL')
        run('validate', work / 'absent', error='E_IO')

    emitted = set()
    for path in (ROOT / 'crates').glob('*/src/*.rs'):
        emitted.update(re.findall(r'"(E_[A-Z_]+)"', path.read_text()))
    documented = set(re.findall(r'`(E_[A-Z_]+)`', (ROOT / 'spec/ERRORS.md').read_text()))
    assert emitted == documented, (emitted - documented, documented - emitted)
    print(f'Public contract: PASS ({calls} CLI calls, two Draft 7 schemas, independent Python producer, error catalog)')


if __name__ == '__main__':
    main()

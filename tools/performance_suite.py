#!/usr/bin/env python3
"""Content-verified local replay store. No build, download, or game mutation."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import contextlib
import csv
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import struct
import tempfile
import time

SCHEMA = 1
HERE = Path(__file__).resolve().parent


def digest(path):
    h = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()


def encoded(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':')).encode()


def write_json(path, value):
    Path(path).write_text(json.dumps(value, indent=2, sort_keys=True) + '\n')


def files(root):
    if any(p.is_symlink() for p in root.rglob('*')):
        raise ValueError('artifact symlinks are not supported')
    return {str(p.relative_to(root)): {'sha256': digest(p), 'bytes': p.stat().st_size}
            for p in sorted(root.rglob('*')) if p.is_file() and not p.is_symlink()}


def cue_files(path):
    """Hash the complete disc dependency closure, including multi-file CUEs."""
    if path.suffix.lower() != '.cue':
        return []
    result = []
    for line in path.read_text().splitlines():
        parts = shlex.split(line)
        if parts and parts[0].upper() == 'FILE':
            if len(parts) != 3:
                raise ValueError('malformed CUE FILE line')
            result.append((path.parent / parts[1]).resolve(strict=True))
    if not result:
        raise ValueError('CUE has no FILE inputs')
    return result


def materialize(case, bindings):
    inputs = {}
    for name, value in case['inputs'].items():
        if not isinstance(value, str) or not value.startswith('$'):
            raise ValueError('input paths must use external $bindings')
        inputs[name] = str(Path(bindings[value[1:]]).expanduser().resolve(strict=True))
    return inputs


# Deliberately bounded to the documented suite lanes. Add new flags here with
# their input/output semantics instead of permitting invisible file inputs.
INPUT_FLAGS = {'--path', '--disc', '--input-tape', '--load-state'}
OUTPUT_FLAGS = {'--config-dir', '--route-log', '--cpu-cycle-profile-log',
                '--gpu-frame-stats-log', '--cd-command-log', '--profile-log',
                '--counter-log', '--route-screenshot-dir', '--dump-display',
                '--dump-ram', '--dump-spu-ram', '--dump-audio', '--dump-vram',
                '--pc-line-log', '--pc-sample-log', '--icache-event-log',
                '--instruction-class-log', '--stack-profile-log'}
SCALAR_FLAGS = {'--steps', '--stop-at-poll', '--guest-frames',
                '--route-screenshot-interval', '--pc-sample-instructions',
                '--pc-line-start-route-tick', '--icache-event-start-route-tick',
                '--stack-profile-root-pc', '--press'}
BOOL_FLAGS = {'--embedded-playtest', '--digital-pad', '--dump-hash',
              '--guest-debug-log', '--dump-guest-profile'}


def check_arguments(argv, inputs):
    if not argv or argv[0] not in ('launch', '{input.driver}'):
        raise ValueError('expected launch or an explicitly hashed test driver')
    index = 1
    while index < len(argv):
        flag = argv[index]; index += 1
        if flag in BOOL_FLAGS:
            continue
        if flag not in INPUT_FLAGS | OUTPUT_FLAGS | SCALAR_FLAGS:
            raise ValueError(f'unsupported option (declare its file semantics first): {flag}')
        if index == len(argv):
            raise ValueError(f'missing value for {flag}')
        value = argv[index]; index += 1
        if flag in INPUT_FLAGS:
            match = re.fullmatch(r'\{input\.([a-zA-Z0-9_]+)\}', value)
            if not match or match[1] not in inputs:
                raise ValueError(f'{flag} must reference one hashed input')
        elif flag in OUTPUT_FLAGS:
            if not value.startswith('{out}/') or '..' in Path(value).parts:
                raise ValueError(f'{flag} must remain under the isolated output directory')


def poll_tape_end(path):
    """Conservative poll completion supports exact binary v2 tapes only."""
    raw = Path(path).read_bytes()
    if len(raw) < 16 or raw[:8] != b'PXITAPE2':
        raise ValueError('poll completion requires a PXITAPE2 tape; use a semantic adapter otherwise')
    count, start = struct.unpack_from('<II', raw, 8)
    if len(raw) != 16 + count * 6:
        raise ValueError('truncated or malformed poll tape')
    return start + count


def plan(case, bindings):
    if case.get('lane') not in ('quick', 'acceptance'):
        raise ValueError('lane must be quick or acceptance')
    inputs = materialize(case, bindings)
    if 'emulator' not in inputs:
        raise ValueError('an explicit emulator binary is required')
    argv = case['argv']
    check_arguments(argv, inputs)
    if not case.get('build_provenance'):
        raise ValueError('record build directory, flags and source provenance in build_provenance')
    for name in case['required_outputs']:
        if Path(name).is_absolute() or '..' in Path(name).parts:
            raise ValueError('required outputs must remain inside the result directory')
    for flag in ('--config-dir', '--steps'):
        if flag not in argv:
            raise ValueError(f'missing {flag}')
    if argv[argv.index('--config-dir') + 1] != '{out}/config':
        raise ValueError('config-dir must be fresh {out}/config')
    if '--path' not in argv:
        raise ValueError('explicit --path is required; library lookup is not reproducible')
    if '--input-tape' in argv and argv[argv.index('--input-tape') + 1] != '{input.tape}':
        raise ValueError('tape must be a hashed tape input')
    if '--path' in argv and not argv[argv.index('--path') + 1].startswith('{input.'):
        raise ValueError('guest path must reference a hashed input')
    if '--disc' in argv and not argv[argv.index('--disc') + 1].startswith('{input.'):
        raise ValueError('disc must reference a hashed input')
    if 'poll' in case['completion']:
        if '--guest-frames' in argv:
            raise ValueError('poll completion forbids competing frame limits')
        maximum = case['completion'].get('max_poll', case['completion']['poll'])
        if 'tape' not in inputs or poll_tape_end(inputs['tape']) <= maximum:
            raise ValueError('tape must extend strictly beyond maximum completion poll')
    environment = case.get('environment', {})
    if any(not key.startswith('PSOXIDE_') for key in environment):
        raise ValueError('only explicit PSOXIDE_* runtime overrides are supported')
    records = {}
    for name, value in inputs.items():
        path = Path(value)
        if not path.is_file():
            raise ValueError(f'{name} is not a file')
        records[name] = {'path': value, 'sha256': digest(path), 'bytes': path.stat().st_size,
                         'cue_dependencies': [{ 'path': str(p), 'sha256': digest(p),
                                                'bytes': p.stat().st_size} for p in cue_files(path)]}
    # Scripts used as executables must declare their interpreter. No opaque shell wrappers.
    with Path(inputs['emulator']).open('rb') as stream:
        executable_magic = stream.read(2)
    if executable_magic == b'#!':
        raise ValueError('emulator must be a real binary, not an unhashed wrapper')
    identity = {'schema': SCHEMA, 'case': case, 'inputs': records,
                'tool_sha256': digest(__file__), 'parser_sha256': digest(HERE / 'cortex_bench_report.py'),
                'runtime_environment': environment, 'config_policy': 'fresh empty directory; absent memory cards',
                'host_platform': os.uname().sysname + '/' + os.uname().machine}
    key = hashlib.sha256(encoded(identity)).hexdigest()
    return {'key': key, 'identity': identity, 'inputs': inputs}


def expand(argv, inputs, out):
    result = []
    for word in argv:
        word = word.replace('{out}', str(out))
        for name, path in inputs.items():
            word = word.replace('{input.' + name + '}', path)
        if '{input.' in word or '{out}' in word:
            raise ValueError('unresolved command placeholder')
        result.append(word)
    return result


def parse_summary(out):
    # Share the existing Cortex stdout parser; do not inherit its aggregate FPS inference.
    from cortex_bench_report import parse_launch_summary, CYCLE_COLUMNS
    summary = parse_launch_summary((out / 'stdout.txt').read_text(errors='replace'))
    cycle_file = out / 'cycles.csv'
    if cycle_file.is_file():
        totals = {}
        with cycle_file.open() as stream:
            for row in csv.DictReader(stream):
                for key, value in row.items():
                    if key in CYCLE_COLUMNS + ['profiled_cpu_cycles', 'uncached_fetch_stall_cycles', 'other_cycles'] and value:
                        totals[key] = totals.get(key, 0) + int(value)
        summary['guest_cycle_columns'] = totals
        summary['cycle_note'] = 'stack_ram_load_stall_cycles is a subset, never add it twice'
    return summary


def completion(case, inputs, out, summary):
    text = (out / 'stdout.txt').read_text(errors='replace')
    if re.search(r'\[cli\] (?:step \d+ failed|guest exception at step)', text):
        raise ValueError('guest fault, not successful completion')
    cap = int(case['argv'][case['argv'].index('--steps') + 1])
    if summary['instructions'] >= cap:
        raise ValueError('instruction hard cap reached, not a completed route')
    expected = case['completion']
    if 'poll' in expected:
        if summary.get('stopped_at') is None or summary['stopped_at'] >= cap:
            raise ValueError('missing successful early-stop marker')
        target = expected['poll']
        if '--stop-at-poll' not in case['argv'] or int(case['argv'][case['argv'].index('--stop-at-poll') + 1]) != target:
            raise ValueError('completion poll must match --stop-at-poll')
        maximum = expected.get('max_poll', target)
        if not target <= summary['port1_polls'] <= maximum:
            raise ValueError(f'completion poll outside explicit bounds: {summary["port1_polls"]}')
        return {'complete': True, 'observed_poll': summary['port1_polls'], 'target': target,
                'note': 'CLI completes after the next display-origin change; bounds are case-specific'}
    adapter = inputs[expected['adapter']]
    interpreter = inputs[expected['interpreter']]
    write_json(out / 'resolved-inputs.json', inputs)
    result = subprocess.run([interpreter, adapter, str(out), str(out / 'resolved-inputs.json')],
                            check=True, capture_output=True, text=True, timeout=30)
    proof = json.loads(result.stdout)
    if proof.get('complete') is not True:
        raise ValueError('semantic completion adapter did not confirm completion')
    return proof


def validate_result(entry, expected_key=None):
    """Rehash every recorded artifact; metadata/mtime alone never proves a hit."""
    receipt = json.loads((entry / 'receipt.json').read_text())
    if receipt.get('status') != 'complete' or receipt.get('schema') != SCHEMA:
        raise ValueError('partial or unknown result')
    identity = receipt['identity']
    key = hashlib.sha256(encoded(identity)).hexdigest()
    if key != receipt['key'] or (expected_key and key != expected_key):
        raise ValueError('result identity mismatch')
    if not receipt['completion'].get('complete') or receipt['returncode'] != 0:
        raise ValueError('result has no successful completion proof')
    actual = files(entry / 'artifacts')
    if actual != receipt['artifacts']:
        raise ValueError('cached artifact missing, changed, or added')
    for name in ('summary', 'completion'):
        if receipt[name] != json.loads((entry / 'artifacts' / (name + '.json')).read_text()):
            raise ValueError('receipt disagrees with sealed artifact proof')
    return receipt


@contextlib.contextmanager
def key_lock(store, key):
    (store / 'locks').mkdir(parents=True, exist_ok=True)
    with (store / 'locks' / (key + '.lock')).open('a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        yield


def run_case(case, bindings, store):
    started = time.perf_counter()
    prepared = plan(case, bindings)
    key = prepared['key']; entry = store / 'results' / key
    with key_lock(store, key):
        if plan(case, bindings)['key'] != key:
            raise ValueError('input changed while waiting for cache lock; retry with stable inputs')
        if entry.exists():
            receipt = validate_result(entry, key)
            return {'case': case['id'], 'key': key, 'cache': 'hit', 'lookup_seconds': time.perf_counter()-started,
                    'result': str(entry), 'summary': receipt['summary']}
        entry.parent.mkdir(parents=True, exist_ok=True)
        temp = Path(tempfile.mkdtemp(prefix=key + '.', dir=entry.parent))
        out = temp / 'artifacts'; out.mkdir(); (out / 'config').mkdir()
        command = [prepared['inputs']['emulator'], *expand(case['argv'], prepared['inputs'], out)]
        # Avoid hidden HOME settings, inherited PSOXIDE_* experiments, and inherited memory cards.
        env = {'PATH': os.defpath, 'HOME': str(out / 'home'), 'TMPDIR': str(out / 'tmp'),
               'LANG': 'C', **case.get('environment', {})}
        (out / 'home').mkdir(); (out / 'tmp').mkdir()
        write_json(out / 'command.json', {'argv': command, 'environment': env})
        wall = time.perf_counter()
        try:
            with (out / 'stdout.txt').open('w') as log:
                result = subprocess.run(command, cwd=out, env=env, stdout=log, stderr=subprocess.STDOUT,
                                        timeout=case.get('timeout_seconds', 900))
            elapsed = time.perf_counter() - wall
            if result.returncode:
                raise ValueError(f'emulator exited {result.returncode}')
            for name in case['required_outputs']:
                path = out / name
                if not path.is_file() or path.stat().st_size == 0:
                    raise ValueError(f'missing required artifact: {name}')
            summary = parse_summary(out)
            proof = completion(case, prepared['inputs'], out, summary)
            # Catch inputs changed during execution rather than caching a mixed run.
            if plan(case, bindings)['key'] != key:
                raise ValueError('input changed during replay')
            write_json(out / 'summary.json', summary)
            write_json(out / 'completion.json', proof)
            receipt = {'schema': SCHEMA, 'status': 'complete', 'key': key,
                       'identity': prepared['identity'], 'returncode': result.returncode,
                       'completion': proof, 'summary': summary, 'host_wall_seconds': elapsed,
                       'artifacts': files(out)}
            write_json(temp / 'receipt.json', receipt)
            temp.rename(entry)
            validate_result(entry, key)
        except Exception as error:
            write_json(temp / 'failure.json', {'status': 'failed', 'error': str(error), 'key': key})
            failed = store / 'failures'; failed.mkdir(exist_ok=True)
            temp.rename(failed / temp.name)
            raise
    return {'case': case['id'], 'key': key, 'cache': 'miss', 'lookup_seconds': time.perf_counter()-started,
            'result': str(entry), 'summary': summary}


def compare(left, right):
    a = validate_result(left); b = validate_result(right)
    ia, ib = a['identity'], b['identity']
    allowed = set(ia['case'].get('candidate_inputs', []))
    if allowed != set(ib['case'].get('candidate_inputs', [])):
        raise ValueError('A/B allowed candidate inputs differ')
    if allowed & {'emulator', 'tape', 'python', 'driver'}:
        raise ValueError('runtime/control inputs cannot vary across A/B')
    for identity in (ia, ib):
        completion_spec = identity['case']['completion']
        if allowed & {completion_spec.get('adapter'), completion_spec.get('interpreter')}:
            raise ValueError('completion implementation cannot vary across A/B')
    for name in set(ia['inputs']) | set(ib['inputs']):
        if name not in allowed and ia['inputs'].get(name) != ib['inputs'].get(name):
            raise ValueError(f'A/B non-candidate input {name} differs')
    for field in ('tool_sha256', 'parser_sha256', 'runtime_environment', 'config_policy'):
        if ia[field] != ib[field]:
            raise ValueError(f'A/B {field} differs')
    for field in ('argv', 'completion', 'lane', 'quality'):
        if ia['case'].get(field) != ib['case'].get(field):
            raise ValueError(f'A/B {field} differs')
    quick = ia['case']['lane'] == 'quick'
    gates = {}
    for dimension in ('visual', 'gameplay', 'audio'):
        spec = ia['case'].get('quality', {}).get(dimension)
        if quick or not spec:
            gates[dimension] = {'status': 'unavailable', 'reason': 'quick timing lane' if quick else 'no adapter evidence declared'}
            continue
        patterns = spec['patterns']; matched = {}
        for root, receipt, tag in ((left, a, 'baseline'), (right, b, 'candidate')):
            matched[tag] = {n: v for n, v in receipt['artifacts'].items()
                            if any(Path(n).match(pattern) for pattern in patterns)}
        floor = 2 if dimension == 'visual' else 1
        minimum = spec.get('minimum_count', floor)
        if not isinstance(minimum, int) or minimum < floor:
            raise ValueError('quality minimum_count is below its evidence floor')
        if min(map(len, matched.values())) < minimum:
            gates[dimension] = {'status': 'unavailable', 'reason': 'insufficient recorded checkpoints'}
        else:
            gates[dimension] = {'status': 'pass' if matched['baseline'] == matched['candidate'] else 'different',
                                'method': 'byte-exact declared artifacts', 'scope': spec['scope'],
                                'baseline_count': len(matched['baseline']), 'candidate_count': len(matched['candidate'])}
    return {'baseline': a['key'], 'candidate': b['key'], 'quality': gates,
            'guest': {'baseline': a['summary'], 'candidate': b['summary']},
            'host_wall_seconds': {'baseline': a['host_wall_seconds'], 'candidate': b['host_wall_seconds'],
                                  'note': 'host elapsed time is not guest performance'},
            'acceptance': 'pass' if all(g['status'] == 'pass' for g in gates.values()) else 'not established',
            'noise': 'No universal threshold; compare a separately recorded behavior-neutral control.'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='action', required=True)
    run = sub.add_parser('run'); run.add_argument('manifest', type=Path); run.add_argument('--bindings', type=Path, required=True)
    run.add_argument('--store', type=Path, required=True); run.add_argument('--jobs', type=int, default=1)
    run.add_argument('--case', action='append'); run.add_argument('--report', type=Path)
    get = sub.add_parser('get'); get.add_argument('result', type=Path)
    comp = sub.add_parser('compare'); comp.add_argument('baseline', type=Path); comp.add_argument('candidate', type=Path)
    args = parser.parse_args()
    if args.action == 'run':
        manifest = json.loads(args.manifest.read_text()); bindings = json.loads(args.bindings.read_text())
        if manifest.get('schema') != SCHEMA or not 1 <= args.jobs <= 8:
            parser.error('schema must be 1; jobs must be between 1 and 8')
        cases = [c for c in manifest['cases'] if not args.case or c['id'] in args.case]
        if not cases or (args.case and set(args.case) != {c['id'] for c in cases}):
            parser.error('unknown or empty case selection')
        with ThreadPoolExecutor(max_workers=args.jobs) as pool:
            result = list(pool.map(lambda c: run_case(c, bindings, args.store.resolve()), cases))
        if args.report:
            write_json(args.report, result)
    elif args.action == 'get':
        t = time.perf_counter(); r = validate_result(args.result)
        result = {'key': r['key'], 'summary': r['summary'], 'artifacts': r['artifacts'],
                  'verification_seconds': time.perf_counter()-t, 'input_verification': 'historical identity only; use run to hash current inputs', 'artifact_root': str(args.result / 'artifacts')}
    else:
        result = compare(args.baseline, args.candidate)
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()

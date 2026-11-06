#!/usr/bin/env python3
"""Offline, immutable backup generations for a stopped UNGE persistent host (Unix)."""
import argparse
from contextlib import ExitStack
from datetime import datetime, timezone
import hashlib
import gzip
import zlib
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import stat
import tempfile

CHUNK = 1024 * 1024
MAX_MANIFEST = 1024 * 1024


def digest_file(source, destination=None, max_bytes=None):
    digest = hashlib.sha256()
    size = 0
    while data := source.read(CHUNK if max_bytes is None else min(CHUNK, max_bytes - size + 1)):
        size += len(data)
        if max_bytes is not None and size > max_bytes:
            raise ValueError('backup exceeds declared size')
        digest.update(data)
        if destination is not None:
            destination.write(data)
    return size, digest.hexdigest()


def regular(path):
    # Never follow a final symlink; callers supply trusted parent directories.
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    stream = os.fdopen(fd, 'rb')
    if not stat.S_ISREG(os.fstat(fd).st_mode):
        stream.close()
        raise ValueError('expected a regular file')
    return stream


def sync_directory(path):
    fd = os.open(path, os.O_RDONLY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def publish(destination, writer):
    destination = Path(destination).absolute()
    # An exclusively reserved destination prevents replacement of another generation.
    destination.mkdir(mode=0o700)
    stage = None
    published = False
    try:
        stage = Path(tempfile.mkdtemp(prefix='.unge-backup-', dir=destination))
        writer(stage)
        sync_directory(stage)
        # Consumers use only destination/generation; partial work stays hidden.
        stage.rename(destination / 'generation')
        published = True
        sync_directory(destination)
        sync_directory(destination.parent)
    finally:
        if stage is not None and stage.exists():
            shutil.rmtree(stage)
        if not published:
            destination.rmdir()


def create(destination, document, state, replay, resources=(), compress=False):
    import fcntl  # Explicitly Unix-only; no unlocked fallback.
    inputs = [('document', Path(document)), ('state', Path(state)), ('replay', Path(replay))]
    inputs.extend((f'resource-{i:06d}', Path(path)) for i, path in enumerate(resources))
    if len(inputs) > 4096:
        raise ValueError('too many files')
    with ExitStack() as stack:
        streams = [(role, path, stack.enter_context(regular(path))) for role, path in inputs]
        identities = [(os.fstat(f.fileno()).st_dev, os.fstat(f.fileno()).st_ino) for _, _, f in streams]
        if len(set(identities)) != len(identities):
            raise ValueError('duplicate source files')
        # Same advisory flock family as Rust File::try_lock on Unix. Keep both
        # log locks and every source descriptor alive until publication completes.
        for _, _, stream in sorted(streams, key=lambda item: str(item[1].absolute())):
            fcntl.flock(stream, fcntl.LOCK_EX | fcntl.LOCK_NB)
        def write(stage):
            entries = []
            stamps = []
            for index, (role, path, source) in enumerate(streams):
                before = os.fstat(source.fileno())
                name = f'{index:06d}.bin' + ('.gz' if compress else '')
                with (stage / name).open('xb') as target:
                    os.chmod(stage / name, 0o600)
                    if compress:
                        with gzip.GzipFile(filename='', fileobj=target, mode='wb', compresslevel=6, mtime=0) as encoded:
                            size, digest = digest_file(source, encoded)
                    else:
                        size, digest = digest_file(source, target)
                    target.flush()
                    os.fsync(target.fileno())
                after = os.fstat(source.fileno())
                current = path.stat(follow_symlinks=False)
                stamp = lambda s: (s.st_dev, s.st_ino, s.st_size, s.st_mtime_ns, s.st_ctime_ns)
                if stamp(before) != stamp(after) or stamp(after) != stamp(current):
                    raise ValueError('source changed during backup')
                stamps.append(stamp(after))
                entry = dict(role=role, file=name, name=path.name, bytes=size, sha256=digest)
                if compress:
                    with regular(stage / name) as stored:
                        stored_bytes, stored_hash = digest_file(stored)
                    entry.update(encoding='gzip', stored_bytes=stored_bytes, stored_sha256=stored_hash)
                entries.append(entry)
            for (_, path, source), saved in zip(streams, stamps):
                if stamp(os.fstat(source.fileno())) != saved or stamp(path.stat(follow_symlinks=False)) != saved:
                    raise ValueError('source changed during backup')
            manifest = dict(version=2 if compress else 1, created_utc=datetime.now(timezone.utc).isoformat(), files=entries)
            data = json.dumps(manifest, ensure_ascii=False, indent=2).encode() + b'\n'
            if len(data) > MAX_MANIFEST:
                raise ValueError('manifest too large')
            with (stage / 'manifest.json').open('xb') as out:
                os.chmod(stage / 'manifest.json', 0o600)
                out.write(data)
                out.flush()
                os.fsync(out.fileno())
        publish(destination, write)


def inspect(generation):
    generation = Path(generation)
    if generation.is_symlink() or not generation.is_dir():
        raise ValueError('expected a generation directory')
    with regular(generation / 'manifest.json') as source:
        data = source.read(MAX_MANIFEST + 1)
    if len(data) > MAX_MANIFEST:
        raise ValueError('manifest too large')
    manifest = json.loads(data)
    if not isinstance(manifest, dict) or set(manifest) != {'version', 'created_utc', 'files'} or type(manifest['version']) is not int or manifest['version'] not in (1, 2) or not isinstance(manifest['created_utc'], str):
        raise ValueError('unsupported manifest')
    entries = manifest['files']
    if not isinstance(entries, list) or not 3 <= len(entries) <= 4096:
        raise ValueError('invalid file count')
    expected = {'manifest.json'}
    for index, entry in enumerate(entries):
        role = ['document', 'state', 'replay'][index] if index < 3 else f'resource-{index - 3:06d}'
        fields = {'role', 'file', 'name', 'bytes', 'sha256'}
        if manifest['version'] == 2: fields |= {'encoding', 'stored_bytes', 'stored_sha256'}
        if not isinstance(entry, dict) or set(entry) != fields:
            raise ValueError('invalid entry')
        name = f'{index:06d}.bin' + ('.gz' if manifest['version'] == 2 else '')
        if entry['role'] != role or entry['file'] != name:
            raise ValueError('invalid role or path')
        if not isinstance(entry['name'], str) or PurePosixPath(entry['name']).name != entry['name'] or not entry['name']:
            raise ValueError('invalid original name')
        if type(entry['bytes']) is not int or entry['bytes'] < 0 or not valid_hash(entry['sha256']):
            raise ValueError('invalid integrity fields')
        if manifest['version'] == 2 and (entry['encoding'] != 'gzip' or type(entry['stored_bytes']) is not int or entry['stored_bytes'] < 20 or not valid_hash(entry['stored_sha256'])):
            raise ValueError('invalid encoding fields')
        expected.add(entry['file'])
    if {p.name for p in generation.iterdir()} != expected:
        raise ValueError('missing or unexpected generation files')
    return manifest


def valid_hash(value):
    return isinstance(value, str) and len(value) == 64 and all(c in '0123456789abcdef' for c in value)


def check_budget(manifest, max_output_bytes):
    if max_output_bytes is not None:
        if type(max_output_bytes) is not int or max_output_bytes < 1:
            raise ValueError('max output bytes must be positive')
        if sum(entry['bytes'] for entry in manifest['files']) > max_output_bytes:
            raise ValueError('backup exceeds output budget')


def checked_copy(generation, entry, destination=None):
    with regular(Path(generation) / entry['file']) as source:
        if entry.get('encoding') == 'gzip':
            if digest_file(source, max_bytes=entry['stored_bytes']) != (entry['stored_bytes'], entry['stored_sha256']):
                raise ValueError('stored backup integrity mismatch')
            source.seek(0)
            try:
                with gzip.GzipFile(fileobj=source, mode='rb') as decoded:
                    result = digest_file(decoded, destination, entry['bytes'])
            except (gzip.BadGzipFile, EOFError, zlib.error) as error:
                raise ValueError('invalid compressed backup') from error
        else:
            result = digest_file(source, destination, entry['bytes'])
        if result != (entry['bytes'], entry['sha256']):
            raise ValueError('backup integrity mismatch')


def verify(generation, max_output_bytes=None):
    manifest = inspect(generation)
    check_budget(manifest, max_output_bytes)
    for entry in manifest['files']:
        checked_copy(generation, entry)
    return manifest


def restore(generation, destination, max_output_bytes=None):
    manifest = inspect(generation)
    check_budget(manifest, max_output_bytes)
    def write(stage):
        entries = []
        for index, entry in enumerate(manifest['files']):
            # Write canonical plain files for existing Provider/Engine startup.
            name = f'{index:06d}.bin'
            with (stage / name).open('xb') as out:
                os.chmod(stage / name, 0o600)
                checked_copy(generation, entry, out)
                out.flush()
                os.fsync(out.fileno())
            entries.append({**{k: entry[k] for k in ('role', 'name', 'bytes', 'sha256')}, 'file': name})
        restored = dict(version=1, created_utc=manifest['created_utc'], files=entries)
        with (stage / 'manifest.json').open('xb') as out:
            os.chmod(stage / 'manifest.json', 0o600)
            out.write(json.dumps(restored, ensure_ascii=False, indent=2).encode() + b'\n')
            out.flush()
            os.fsync(out.fileno())
    publish(destination, write)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    new = commands.add_parser('create')
    new.add_argument('destination', type=Path)
    for name in ('document', 'state', 'replay'):
        new.add_argument('--' + name, required=True, type=Path)
    new.add_argument('--resource', action='append', default=[], type=Path)
    new.add_argument('--compress', action='store_true', help='gzip payloads using manifest version 2')
    check = commands.add_parser('verify')
    check.add_argument('generation', type=Path)
    load = commands.add_parser('restore')
    load.add_argument('generation', type=Path)
    load.add_argument('destination', type=Path)
    for command in (check, load):
        command.add_argument('--max-output-bytes', type=int, help='maximum total uncompressed payload bytes')
    args = parser.parse_args()
    if os.name != 'posix':
        parser.exit(1, 'backup requires Unix file locking and directory sync\n')
    try:
        if args.command == 'create':
            create(args.destination, args.document, args.state, args.replay, args.resource, compress=args.compress)
        elif args.command == 'verify':
            verify(args.generation, args.max_output_bytes)
        else:
            restore(args.generation, args.destination, args.max_output_bytes)
    except (OSError, ValueError, TypeError, KeyError, ImportError) as error:
        parser.exit(1, f'backup failed: {error}\n')
    print('OK')


if __name__ == '__main__':
    main()

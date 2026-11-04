#!/usr/bin/env python3
"""Backup integrity/failure checks and real Rust writer/replay interoperability."""
import fcntl
import importlib.util
import json
from pathlib import Path
import tempfile
import subprocess
import sys
import unittest
import uuid
from unittest.mock import patch
import backup

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location('agent', ROOT / 'examples/acx-provider/agent.py')
agent = importlib.util.module_from_spec(spec)
spec.loader.exec_module(agent)


class BackupTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.inputs = [self.root / name for name in ('seed', 'state', 'replay', 'asset')]
        for index, path in enumerate(self.inputs):
            path.write_bytes(bytes([index]) * (backup.CHUNK + 17))

    def create(self):
        dest = self.root / 'backup'
        backup.create(dest, *self.inputs[:3], self.inputs[3:])
        return dest / 'generation'

    def test_roundtrip_private_modes_and_no_overwrite(self):
        generation = self.create()
        manifest = backup.verify(generation)
        process = subprocess.run([sys.executable, str(ROOT / 'scripts/backup.py'), 'verify', str(generation)], capture_output=True, text=True)
        self.assertEqual(process.returncode, 0, process.stderr)
        self.assertEqual(process.stdout.strip(), 'OK')
        self.assertEqual(len(manifest['files']), 4)
        backup.restore(generation, self.root / 'restored')
        for entry, source in zip(manifest['files'], self.inputs):
            target = self.root / 'restored/generation' / entry['file']
            self.assertEqual(target.read_bytes(), source.read_bytes())
            self.assertEqual(target.stat().st_mode & 0o777, 0o600)
        with self.assertRaises(FileExistsError):
            backup.restore(generation, self.root / 'restored')
        with self.assertRaises(FileExistsError):
            backup.create(self.root / 'backup', *self.inputs[:3])

    def test_tampering_missing_and_extra_files_rejected(self):
        generation = self.create()
        data = generation / '000001.bin'
        original = data.read_bytes()
        data.write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError, 'integrity'):
            backup.verify(generation)
        with self.assertRaises(ValueError):
            backup.restore(generation, self.root / 'bad')
        self.assertFalse((self.root / 'bad').exists())
        data.write_bytes(original)
        (generation / 'extra').touch()
        with self.assertRaises(ValueError): backup.verify(generation)
        (generation / 'extra').unlink()
        data.unlink()
        with self.assertRaises(ValueError): backup.verify(generation)

    def test_lock_missing_symlink_and_duplicate_sources(self):
        with self.inputs[1].open('rb') as stream:
            fcntl.flock(stream, fcntl.LOCK_EX | fcntl.LOCK_NB)
            with self.assertRaises(BlockingIOError): self.create()
        self.assertFalse((self.root / 'backup').exists())
        with self.assertRaises(ValueError):
            backup.create(self.root / 'duplicate', self.inputs[0], self.inputs[1], self.inputs[1])
        link = self.root / 'link'
        link.symlink_to(self.inputs[0])
        with self.assertRaises(OSError):
            backup.create(self.root / 'linked', link, *self.inputs[1:3])
        self.inputs[1].unlink()
        with self.assertRaises(FileNotFoundError): self.create()

    def test_failed_sync_and_midcopy_change_leave_no_generation(self):
        with patch.object(backup.os, 'fsync', side_effect=OSError('disk failure')):
            with self.assertRaises(OSError): self.create()
        self.assertFalse((self.root / 'backup').exists())
        original = backup.digest_file
        def change(source, target=None):
            result = original(source, target)
            self.inputs[0].write_bytes(b'changed externally')
            return result
        with patch.object(backup, 'digest_file', side_effect=change):
            with self.assertRaisesRegex(ValueError, 'source changed'): self.create()
        self.assertFalse((self.root / 'backup').exists())

    def test_manifest_version_paths_and_limits(self):
        generation = self.create()
        path = generation / 'manifest.json'
        original = path.read_bytes()
        for mutate in (lambda m: m.update(version=2), lambda m: m['files'][0].update(file='../escape'),
                       lambda m: m['files'][1].update(role='replay'), lambda m: m['files'][0].update(bytes=True)):
            value = json.loads(original)
            mutate(value)
            path.write_text(json.dumps(value))
            with self.assertRaises(ValueError): backup.verify(generation)
        path.write_bytes(b' ' * (backup.MAX_MANIFEST + 1))
        with self.assertRaisesRegex(ValueError, 'too large'): backup.verify(generation)

    def test_rust_writer_lock_and_restored_receipt_over_stdio(self):
        document, state, replay = self.inputs[:3]
        document.write_text(json.dumps({'schema_version':1,'engine_version':'0.1.0','plugin_versions':{},
            'graph':{'id':str(uuid.uuid4()),'name':'backup test','nodes':{},'edges':{},'groups':{}},'placement':{}}))
        state.unlink()
        replay.unlink()
        args = ['--document',str(document),'--state-log',str(state),'--journal',str(replay)]
        client = agent.Agent(ROOT / 'target/debug/unge-acx-provider', extra_args=args)
        try:
            before = client.call('observe', {'query':'summary'})
            with self.assertRaises(BlockingIOError): self.create()
            intent = {'kind':'edit','document_id':before['documentId'],'expected_revision':0,
                'operations':[{'kind':'create_node','id':str(uuid.uuid4()),'type_id':'math.number',
                    'properties':{'value':42},'rect':{'x':0,'y':0,'width':180,'height':90}}]}
            pf = client.call('preflight', {'input':intent})
            bound = {'preflightId':pf['preflightId'],'preflightDigest':pf['preflightDigest']}
            grant = client.call('authorize', bound)
            commit = client.call('commit', {**bound,'input':pf['input'],'authorization':grant['authorization']})
            execute = {'commitId':commit['commitId']}
            result = client.call('execute', execute)
            receipt = client.call('receipt', {'receiptId':result['receiptId']})
            saved = client.call('observe', {'query':'summary'})
        finally:
            client.close()
        generation = self.create()
        backup.restore(generation, self.root / 'restore')
        restored = self.root / 'restore/generation'
        client = agent.Agent(ROOT / 'target/debug/unge-acx-provider', extra_args=[
            '--document',str(restored/'000000.bin'),'--state-log',str(restored/'000001.bin'),
            '--journal',str(restored/'000002.bin')])
        try:
            self.assertEqual(client.call('observe', {'query':'summary'}), saved)
            self.assertEqual(client.call('execute', execute), result)
            self.assertEqual(client.call('receipt', {'receiptId':result['receiptId']}), receipt)
        finally:
            client.close()


if __name__ == '__main__':
    unittest.main()

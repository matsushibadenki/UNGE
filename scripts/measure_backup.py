#!/usr/bin/env python3
"""Measure stored bytes for a synthetic stopped Rust Provider; no timing claims."""
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import uuid
import sys
import zlib
import backup

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location('agent', ROOT/'examples/acx-provider/agent.py')
agent = importlib.util.module_from_spec(spec)
spec.loader.exec_module(agent)


def measure():
    with tempfile.TemporaryDirectory(prefix='unge-backup-measure-') as directory:
        folder = Path(directory)
        seed = folder/'seed.json'
        port = {'name':'value','data_type':{'kind':'float'},'cardinality':'single','required':True}
        nodes = {}
        placement = {}
        for i in range(1, 1001):
            node = str(uuid.UUID(int=i))
            nodes[node] = {'id':node,'type_id':'math.number','inputs':[],'outputs':[port],'properties':{'value':0}}
            placement[node] = {'x':(i%100)*240,'y':(i//100)*140,'width':200,'height':100}
        seed.write_text(json.dumps({'schema_version':1,'engine_version':'0.1.0','plugin_versions':{},
            'graph':{'id':str(uuid.UUID(int=100000)),'name':'compressed backup fixture','nodes':nodes,'edges':{},'groups':{}},'placement':placement}, separators=(',',':')))
        state, replay = folder/'state.jsonl', folder/'replay.jsonl'
        options = ['--document',str(seed),'--state-log',str(state),'--journal',str(replay)]
        client = agent.Agent(ROOT/'target/debug/unge-acx-provider',extra_args=options)
        try:
            summary = client.call('observe',{'query':'summary'})
            for i in range(16):
                intent = {'kind':'edit','document_id':summary['documentId'],'expected_revision':summary['revision'],
                    'operations':[{'kind':'set_property','id':str(uuid.UUID(int=i+1)),'key':'value','value':i+1}]}
                pf = client.call('preflight',{'input':intent})
                bound = {'preflightId':pf['preflightId'],'preflightDigest':pf['preflightDigest']}
                grant = client.call('authorize',bound)
                commit = client.call('commit',{**bound,'input':pf['input'],'authorization':grant['authorization']})
                execute = {'commitId':commit['commitId']}
                result = client.call('execute',execute)
                summary = client.call('observe',{'query':'summary'})
            receipt = client.call('receipt',{'receiptId':result['receiptId']})
        finally:
            client.close()
        assert summary['nodes'] == 1000 and summary['revision'] == 16
        cases = []
        for compressed in (False, True):
            target = folder/('gzip' if compressed else 'plain')
            backup.create(target,seed,state,replay,compress=compressed)
            manifest = backup.verify(target/'generation', 8*1024*1024)
            restored = folder/('decoded-gzip' if compressed else 'decoded-plain')
            backup.restore(target/'generation',restored,8*1024*1024)
            files = []
            for index, path in enumerate((seed,state,replay)):
                data = path.read_bytes()
                assert (restored/'generation'/f'{index:06d}.bin').read_bytes() == data
                files.append({'role':manifest['files'][index]['role'],'original_bytes':len(data),
                    'stored_bytes':(target/'generation'/manifest['files'][index]['file']).stat().st_size,
                    'sha256':hashlib.sha256(data).hexdigest()})
            active = restored/'generation'
            client = agent.Agent(ROOT/'target/debug/unge-acx-provider',extra_args=[
                '--document',str(active/'000000.bin'),'--state-log',str(active/'000001.bin'),'--journal',str(active/'000002.bin')])
            try:
                assert client.call('observe',{'query':'summary'}) == summary
                assert client.call('execute',execute) == result
                assert client.call('receipt',{'receiptId':result['receiptId']}) == receipt
                assert client.call('observe',{'query':'summary'}) == summary
            finally:
                client.close()
            total = sum(p.stat().st_size for p in (target/'generation').iterdir())
            cases.append({'compression':'gzip' if compressed else 'none','manifest_version':manifest['version'],
                'payload_bytes':sum(f['stored_bytes'] for f in files),'generation_bytes_including_manifest':total,
                'files':files,'restored_bytes_equal':True,'provider_revision':summary['revision'],'original_receipt_replayed':True})
        return {'version':1,'python':sys.version.split()[0],'zlib':zlib.ZLIB_RUNTIME_VERSION,'nodes':1000,'edits':16,'edges':0,'resources':0,'compression_level':6,'cases':cases,
            'scope':'Synthetic local archive byte counts, including manifest separately; not latency, cold recovery, active-log compaction or typical-project compression guarantees.'}


if __name__ == '__main__':
    print(json.dumps(measure(),indent=2))

#!/usr/bin/env python3
"""Opt-in real macOS Tauri persistence smoke test; opens native windows."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import uuid

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location('unge_agent', ROOT / 'examples/acx-provider/agent.py')
agent = importlib.util.module_from_spec(spec)
spec.loader.exec_module(agent)


def run(binary, hold=False):
    with tempfile.TemporaryDirectory(prefix='unge-tauri-state-') as directory:
        folder = Path(directory)
        doc = folder / 'seed.json'
        doc.write_text(json.dumps({'schema_version':1,'engine_version':'0.1.0','plugin_versions':{},
            'graph':{'id':str(uuid.uuid4()),'name':'persistent native test','nodes':{},'edges':{},'groups':{}},'placement':{}}))
        # Isolate presentation preferences as well as the two authoritative logs.
        old = os.environ.get('UNGE_PANEL_PREFERENCES')
        os.environ['UNGE_PANEL_PREFERENCES'] = str(folder / 'panels.json')
        options = ['--document',str(doc),'--state-log',str(folder/'state.jsonl'),'--journal',str(folder/'replay.jsonl')]
        client = None
        try:
            client = agent.Agent(binary, desktop=True, extra_args=options)
            before = client.call('observe', {'query':'summary'})
            node = str(uuid.uuid4())
            intent = {'kind':'edit','document_id':before['documentId'],'expected_revision':0,
                'operations':[{'kind':'create_node','id':node,'type_id':'math.number','properties':{'value':42},
                               'rect':{'x':40,'y':40,'width':200,'height':100}}]}
            pf = client.call('preflight', {'input':intent})
            bound = {'preflightId':pf['preflightId'],'preflightDigest':pf['preflightDigest']}
            grant = client.call('authorize', bound)
            commit = client.call('commit', {**bound,'input':pf['input'],'authorization':grant['authorization']})
            execute = {'commitId':commit['commitId']}
            result = client.call('execute', execute)
            receipt = client.call('receipt', {'receiptId':result['receiptId']})
            saved = client.call('observe', {'query':'summary'})
            assert (saved['nodes'],saved['revision']) == (1,1)
            client.close()
            assert client.process.returncode == 0
            client = agent.Agent(binary, desktop=True, extra_args=options)
            assert client.call('observe', {'query':'summary'}) == saved
            assert client.call('execute', execute) == result
            assert client.call('receipt', {'receiptId':result['receiptId']}) == receipt
            assert client.call('observe', {'query':'node','id':node})['node']['node']['properties']['value'] == 42
            agent.Agent.verify_json(result['resultJson'],result['result'],receipt['resultHash'])
            print('Tauri restart: 1 node / revision 1 / value 42 / original receipt replay passed', flush=True)
            if hold:
                input('Native QA is ready; press Enter to close the test app.\n')
            client.close()
            assert client.process.returncode == 0
            client = None
        finally:
            if client is not None and client.process.poll() is None:
                client.close()
            if old is None:
                os.environ.pop('UNGE_PANEL_PREFERENCES', None)
            else:
                os.environ['UNGE_PANEL_PREFERENCES'] = old


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=ROOT/'target/debug/unge-tauri-host')
    parser.add_argument('--hold', action='store_true')
    args = parser.parse_args()
    run(args.binary, args.hold)

#!/usr/bin/env python3
"""Independent Python agent for UNGE's experimental ACX JSON Lines binding."""
import argparse
import hashlib
import json
from pathlib import Path
import queue
import subprocess
import threading
import uuid


class Agent:
    def __init__(self, binary, desktop=False, validate=None):
        self.process = subprocess.Popen([str(binary)] + (['--acx-stdio'] if desktop else []), stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, encoding='utf-8', bufsize=1)
        self.lines = queue.Queue()
        self.sequence = 0
        self.validate = validate or (lambda value, schema: None)
        self.capabilities = {}
        def reader():
            for line in self.process.stdout:
                self.lines.put(line)
            self.lines.put(None)
        threading.Thread(target=reader, daemon=True).start()

    def call(self, method, params=None):
        self.sequence += 1
        correlation = str(self.sequence)
        self.process.stdin.write(json.dumps({'id': correlation, 'method': method, 'params': params or {}}, ensure_ascii=False, allow_nan=False) + '\n')
        self.process.stdin.flush()
        line = self.lines.get(timeout=30)
        if line is None:
            raise RuntimeError('provider closed stdout')
        response = json.loads(line)
        if response['id'] != correlation:
            raise RuntimeError('response correlation mismatch')
        if 'error' in response:
            raise RuntimeError(response['error'])
        return response['result']

    @staticmethod
    def verify_json(text, structured, expected_hash):
        actual = 'sha256:' + hashlib.sha256(text.encode('utf-8')).hexdigest()
        if actual != expected_hash or json.loads(text) != structured:
            raise RuntimeError('ACX evidence mismatch')

    def prepare(self, intent):
        capability = 'org.unge.graph.edit' if intent['kind'] == 'edit' else 'org.unge.graph.run'
        self.validate(intent, self.capabilities[capability]['inputSchema'])
        preflight = self.call('preflight', {'input': intent})
        if preflight['provider'] != self.provider or preflight['capability'] != capability:
            raise RuntimeError('preflight identity mismatch')
        self.verify_json(preflight['requestJson'], preflight['input'], preflight['requestHash'])
        unhashed = {k: v for k, v in preflight.items() if k not in ['preflightJson', 'preflightDigest']}
        self.verify_json(preflight['preflightJson'], unhashed, preflight['preflightDigest'])
        # Geometry is normalized by the provider; confirm semantics before approval.
        if preflight['input'] != intent:
            raise RuntimeError('provider changed the requested intent')
        if preflight['cost'] != {'currency': 'USD', 'estimated': '0.00', 'max': '0.00'}:
            raise RuntimeError('unexpected cost')
        if preflight['approval']['type'] != 'policy':
            raise RuntimeError('unsupported approval policy')
        bound = {'preflightId': preflight['preflightId'], 'preflightDigest': preflight['preflightDigest']}
        authorization = self.call('authorize', bound)
        if authorization['provider'] != self.provider or authorization['scope'] != preflight['approval']['scope'] or authorization['expiresAt'] != preflight['expiresAt']:
            raise RuntimeError('authorization binding mismatch')
        commit = self.call('commit', {**bound, 'authorization': authorization['authorization'], 'input': preflight['input']})
        if commit['preflightId'] != preflight['preflightId']:
            raise RuntimeError('commit binding mismatch')
        return preflight, authorization, commit

    def receipt(self, execution, preflight):
        receipt = self.call('receipt', {'receiptId': execution['receiptId']})
        self.validate(receipt, 'receipt')
        if receipt['provider'] != self.provider or receipt['capability'] != preflight['capability'] or receipt['requestHash'] != preflight['requestHash'] or receipt['receiptId'] != execution['receiptId']:
            raise RuntimeError('receipt binding mismatch')
        self.verify_json(execution['resultJson'], execution['result'], receipt['resultHash'])
        if receipt['status'] != 'succeeded':
            raise RuntimeError('execution did not succeed')
        return receipt

    def close(self):
        self.process.stdin.close()
        try:
            self.process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.process.terminate()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()
        self.process.stdout.close()


def run_demo(binary, desktop=False, validate=None, progress=print):
    agent = Agent(binary, desktop, validate)
    try:
        manifest = agent.call('discover')
        agent.validate(manifest, 'manifest')
        agent.provider = manifest['provider']['id']
        agent.capabilities = {c['id']: c for c in manifest['capabilities']}
        assert {'org.unge.graph.observe', 'org.unge.graph.edit', 'org.unge.graph.run'} <= agent.capabilities.keys()
        progress('discover PASS')
        before = agent.call('observe', {'query': 'summary'})
        definitions = agent.call('observe', {'query': 'definitions'})
        assert len(definitions['items']) == 2
        a, b, total = [str(uuid.uuid4()) for _ in range(3)]
        def create(node_id, type_id, properties, x, y):
            return {'kind': 'create_node', 'id': node_id, 'type_id': type_id, 'properties': properties, 'rect': {'x': x, 'y': y, 'width': 180, 'height': 90}}
        operations = [create(a, 'math.number', {'value': 20, 'label': '東京 / 北京 / café'}, 10.1, 10.2), create(b, 'math.number', {'value': 22}, 10, 160), create(total, 'math.add', {}, 320, 80)]
        for source, port in [(a, 'a'), (b, 'b')]:
            operations.append({'kind': 'connect', 'edge': {'id': str(uuid.uuid4()), 'from': {'node': source, 'port': 'value'}, 'to': {'node': total, 'port': port}}})
        intent = {'kind': 'edit', 'document_id': before['documentId'], 'expected_revision': before['revision'], 'operations': operations}
        pf, grant, commit = agent.prepare(intent)
        assert agent.call('observe', {'query': 'summary'}) == before
        progress('preflight / authorize / commit PASS — document unchanged')
        execution = agent.call('execute', {'commitId': commit['commitId']})
        edit_receipt = agent.receipt(execution, pf)
        agent.validate(execution['result'], agent.capabilities['org.unge.graph.edit']['outputSchema'])
        after = execution['result']['summary']
        assert after['nodes'] == before['nodes'] + 3 and after['edges'] == before['edges'] + 2
        assert agent.call('execute', {'commitId': commit['commitId']}) == execution
        progress('edit / receipt / retry PASS — added 3 nodes and 2 edges once')
        run_intent = {'kind': 'run', 'document_id': after['documentId'], 'expected_revision': after['revision']}
        run_pf, _, run_commit = agent.prepare(run_intent)
        run = agent.call('execute', {'commitId': run_commit['commitId']})
        run_receipt = agent.receipt(run, run_pf)
        agent.validate(run['result'], agent.capabilities['org.unge.graph.run']['outputSchema'])
        assert run['result']['nodes'][total]['outputs']['value'] == {'kind': 'float', 'value': 42.0}
        progress('run PASS — 20 + 22 = 42')
        recovery_args = {'commitId': commit['commitId'], 'authorization': grant['authorization'], 'expectedRevision': after['revision']}
        recovery = agent.call('recover', recovery_args)
        recovery_receipt = agent.receipt(recovery, pf)
        assert recovery_receipt['recovery']['state'] == 'recovered'
        assert recovery['result']['documentHash'] == pf['documentHash']
        assert agent.call('recover', recovery_args) == recovery
        restored = agent.call('observe', {'query': 'summary'})
        assert restored['nodes'] == before['nodes'] and restored['edges'] == before['edges']
        progress('recover PASS — original document restored; receipt verified')
        return {'preflight': pf, 'execution': execution, 'editReceipt': edit_receipt, 'runReceipt': run_receipt, 'recoveryReceipt': recovery_receipt, 'restored': restored}
    finally:
        agent.close()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=Path(__file__).resolve().parents[2] / 'target/debug/unge-acx-provider')
    parser.add_argument('--desktop', action='store_true', help='Launch a Tauri host with --acx-stdio')
    args = parser.parse_args()
    run_demo(args.binary.resolve(), args.desktop)

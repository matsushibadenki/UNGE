#!/usr/bin/env python3
"""Validate Rust ACX output against upstream schemas with an independent Python client."""
import importlib.util
import json
from pathlib import Path
import unittest
import uuid
import tempfile
from jsonschema import Draft202012Validator, FormatChecker

ROOT = Path(__file__).resolve().parent.parent
SPEC = ROOT / 'crates/unge-acx/spec'
spec = importlib.util.spec_from_file_location('unge_acx_agent', ROOT / 'examples/acx-provider/agent.py')
agent = importlib.util.module_from_spec(spec)
spec.loader.exec_module(agent)

class AcxInteropTests(unittest.TestCase):
    def test_rust_provider_full_lifecycle_and_schemas(self):
        def validate(value, schema):
            if isinstance(schema, str):
                schema = json.loads((SPEC / f'acx-{schema}.schema.json').read_text())
            Draft202012Validator.check_schema(schema)
            Draft202012Validator(schema, format_checker=FormatChecker()).validate(value)
        transcript = agent.run_demo(ROOT / 'target/debug/unge-acx-provider', validate=validate, progress=lambda _: None)
        self.assertEqual(transcript['restored']['nodes'], 0)
        self.assertEqual(transcript['restored']['revision'], 2)
    def test_async_job_profile_over_real_stdio(self):
        schema = json.loads((SPEC / 'acx-node-graph-job.schema.json').read_text())
        def validate(value, definition):
            if definition == 'node-graph-job': definition = schema
            elif isinstance(definition, str): definition = json.loads((SPEC / f'acx-{definition}.schema.json').read_text())
            Draft202012Validator(definition, format_checker=FormatChecker()).validate(value)
        result = agent.run_demo(ROOT / 'target/debug/unge-acx-provider', validate=validate, progress=lambda _: None, async_run=True)
        self.assertEqual(result['restored']['nodes'], 0)

    def test_property_metadata_rejection_and_creation_defaults_over_stdio(self):
        client = agent.Agent(ROOT / 'target/debug/unge-acx-provider')
        try:
            definitions = client.call('observe', {'query': 'definitions'})
            number = next(d for d in definitions['items'] if d['type_id'] == 'math.number')
            field = number['property_schema']['fields']['value']
            self.assertEqual(field['value_type']['kind'], 'float')
            self.assertTrue(field['required'])
            self.assertEqual(field['default'], 0)
            self.assertEqual(set(field['name']), {'en', 'ja', 'zh_cn'})
            before = client.call('observe', {'query': 'summary'})
            node_id = str(uuid.uuid4())
            operation = {'kind':'create_node', 'id':node_id, 'type_id':'math.number',
                         'properties': {'value': 'invalid'},
                         'rect': {'x':0, 'y':0, 'width':180, 'height':90}}
            intent = {'kind':'edit', 'document_id':before['documentId'],
                      'expected_revision':before['revision'], 'operations':[operation]}
            with self.assertRaisesRegex(RuntimeError, 'invalid_properties'):
                client.call('preflight', {'input': intent})
            self.assertEqual(client.call('observe', {'query':'summary'}), before)
            operation['properties'] = {}
            pf = client.call('preflight', {'input':intent})
            bound = {'preflightId':pf['preflightId'], 'preflightDigest':pf['preflightDigest']}
            grant = client.call('authorize', bound)
            commit = client.call('commit', {**bound, 'authorization':grant['authorization'], 'input':pf['input']})
            client.call('execute', {'commitId':commit['commitId']})
            observed = client.call('observe', {'query':'node', 'id':node_id})
            self.assertEqual(observed['node']['node']['properties']['value'], 0)
            self.assertEqual(observed['revision'], before['revision'] + 1)
        finally:
            client.close()
    def test_durable_restart_and_host_selected_rotation_over_stdio(self):
        with tempfile.TemporaryDirectory(prefix='unge-durable-') as directory:
            folder = Path(directory)
            document = folder / 'document.json'
            document.write_text(json.dumps({'schema_version':1,'engine_version':'0.1.0','plugin_versions':{},'graph':{'id':str(uuid.uuid4()),'name':'durable test','nodes':{},'edges':{},'groups':{}},'placement':{}}))
            journal = folder / 'replay.jsonl'
            options = ['--document', str(document), '--journal', str(journal)]
            client = agent.Agent(ROOT / 'target/debug/unge-acx-provider', extra_args=options)
            try:
                before = client.call('observe', {'query':'summary'})
                manifest = client.call('discover')
                old_provider = manifest['provider']['id']
                self.assertTrue(manifest['capabilities'][0]['extensions']['org.unge.node-graph']['replay']['durable'])
                intent = {'kind':'edit','document_id':before['documentId'],'expected_revision':0,'operations':[{'kind':'create_node','id':str(uuid.uuid4()),'type_id':'math.number','properties':{'value':42},'rect':{'x':0,'y':0,'width':180,'height':90}}]}
                pf = client.call('preflight', {'input':intent})
                bound = {'preflightId':pf['preflightId'],'preflightDigest':pf['preflightDigest']}
                grant = client.call('authorize', bound)
                commit = client.call('commit', {**bound,'authorization':grant['authorization'],'input':pf['input']})
                result = client.call('execute', {'commitId':commit['commitId']})
                evidence = client.call('receipt', {'receiptId':result['receiptId']})
                agent.Agent.verify_json(result['resultJson'], result['result'], evidence['resultHash'])
                Draft202012Validator(json.loads((SPEC / 'acx-receipt.schema.json').read_text()),format_checker=FormatChecker()).validate(evidence)
            finally:
                client.close()
            for rotation in (False, True):
                client = agent.Agent(ROOT / 'target/debug/unge-acx-provider', extra_args=options + (['--rotate-session'] if rotation else []))
                try:
                    current_provider = client.call('discover')['provider']['id']
                    if rotation: self.assertNotEqual(current_provider, old_provider)
                    else: self.assertEqual(current_provider, old_provider)
                    self.assertEqual(client.call('execute', {'commitId':commit['commitId']}), result)
                    self.assertEqual(client.call('receipt', {'receiptId':result['receiptId']}), evidence)
                    # The demo reloads the host's saved Document. Journal replay must
                    # never reapply the old edit to this restored snapshot.
                    self.assertEqual(client.call('observe', {'query':'summary'}), before)
                finally:
                    client.close()

    def test_tampered_evidence_is_rejected(self):
        with self.assertRaises(RuntimeError):
            agent.Agent.verify_json('{"value":43}', {'value':43}, 'sha256:'+'0'*64)
    def test_vendored_schema_matches_acx_when_available(self):
        upstream = ROOT / 'acx/schemas/profiles/acx-node-graph-intent.schema.json'
        if not upstream.exists():
            self.skipTest('optional ACX checkout absent')
        self.assertEqual(json.loads(upstream.read_text()), json.loads((SPEC / upstream.name).read_text()))
        job = upstream.with_name('acx-node-graph-job.schema.json')
        self.assertEqual(json.loads(job.read_text()), json.loads((SPEC / job.name).read_text()))

if __name__ == '__main__': unittest.main()

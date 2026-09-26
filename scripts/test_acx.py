#!/usr/bin/env python3
"""Validate Rust ACX output against upstream schemas with an independent Python client."""
import importlib.util
import json
from pathlib import Path
import unittest
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
    def test_tampered_evidence_is_rejected(self):
        with self.assertRaises(RuntimeError):
            agent.Agent.verify_json('{"value":43}', {'value':43}, 'sha256:'+'0'*64)
    def test_vendored_schema_matches_acx_when_available(self):
        upstream = ROOT / 'acx/schemas/profiles/acx-node-graph-intent.schema.json'
        if not upstream.exists():
            self.skipTest('optional ACX checkout absent')
        self.assertEqual(json.loads(upstream.read_text()), json.loads((SPEC / upstream.name).read_text()))

if __name__ == '__main__': unittest.main()

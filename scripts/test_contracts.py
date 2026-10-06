#!/usr/bin/env python3
"""Cross-language checks for generated contracts; requires jsonschema 4."""
import json
import subprocess
import unittest
from jsonschema import Draft7Validator, FormatChecker
from generate_contracts import ROOT, artifacts, ts_type


class Contracts(unittest.TestCase):
    def test_port_map_runtime_parity(self):
        count = 0
        for case in self.fixtures["port_cases"]:
            for direction in ("input", "output"):
                schema = case[direction + "_schema"]
                Draft7Validator.check_schema(schema)
                validator = Draft7Validator(schema, format_checker=FormatChecker())
                for sample in case[direction + "s"]:
                    self.assertEqual(validator.is_valid(sample["value"]), sample["valid"], (schema, sample))
                    count += 1
        self.assertGreater(count, 3000)

    @classmethod
    def setUpClass(cls):
        result = subprocess.run(['cargo', 'run', '--quiet', '--locked', '-p', 'unge-contracts', '--', '--fixtures'], cwd=ROOT, check=True, text=True, capture_output=True)
        cls.fixtures = json.loads(result.stdout)

    def validator(self, name):
        schema = json.loads((ROOT / 'bindings/schema' / (name + '.schema.json')).read_text())
        Draft7Validator.check_schema(schema)
        return Draft7Validator(schema, format_checker=FormatChecker())

    def test_real_rust_serialization(self):
        for name in ('Document', 'Definition', 'Command', 'Request'):
            with self.subTest(name=name):
                self.validator(name).validate(self.fixtures[name])
        for path in (ROOT / 'bindings/schema').glob('*.json'):
            Draft7Validator.check_schema(json.loads(path.read_text()))

    def test_rejection_and_serde_defaults(self):
        validator = self.validator('Request')
        self.assertFalse(validator.is_valid({'kind': 'unknown'}))
        self.assertFalse(validator.is_valid({'kind': 'apply', 'command': {'kind': 'batch', 'commands': []}}))
        self.assertFalse(validator.is_valid({'kind': 'set_viewport', 'viewport': {'origin': [1], 'size': [1, 2], 'zoom': 1}}))
        self.assertFalse(validator.is_valid({'kind': 'pointer', 'expected_revision': 0, 'event': {'kind': 'cancel', 'unexpected': 1}}))
        self.assertTrue(self.validator('Command').is_valid({'kind': 'set_property', 'id': self.fixtures['Command']['commands'][1]['id'], 'key': 'value'}))

    def test_property_value_constraints(self):
        schema = self.fixtures['property_values_schema']
        Draft7Validator.check_schema(schema)
        validator = Draft7Validator(schema)
        self.assertTrue(validator.is_valid({'value': 42}))
        self.assertFalse(validator.is_valid({'value': '42'}))
        self.assertFalse(validator.is_valid({'value': None}))
        self.assertTrue(validator.is_valid({'value': 1, 'unexpected': 2}))

    def test_property_validator_parity(self):
        for case in self.fixtures['property_cases']:
            Draft7Validator.check_schema(case['schema'])
            validator = Draft7Validator(case['schema'])
            for sample in case['samples']:
                with self.subTest(schema=case['schema'], value=sample['value']):
                    self.assertEqual(validator.is_valid(sample['value']), sample['valid'])

    def test_typescript_conversion(self):
        self.assertEqual(ts_type({'type': 'array', 'items': {'type': 'number'}, 'minItems': 2, 'maxItems': 2}), '[number, number]')
        self.assertEqual(ts_type({'type': 'object', 'properties': {'x': {'type': ['string', 'null']}}}), '{ "x"?: (string | null); }')
        self.assertEqual(ts_type({'type': 'object', 'properties': {'x': {'type': ['string', 'null']}}}, output=True), '{ "x": (string | null); }')
        with self.assertRaises(ValueError):
            ts_type({'$ref': 'https://example.com/external'})
        with self.assertRaises(ValueError):
            ts_type({'not': {'type': 'string'}})
        with self.assertRaises(ValueError):
            artifacts({'A': {'type': 'string', 'definitions': {'B': {'type': 'number'}}}, 'B': {'type': 'boolean'}})


if __name__ == '__main__':
    unittest.main()

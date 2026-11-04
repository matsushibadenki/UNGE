#!/usr/bin/env python3
"""Generate portable contracts from Rust. --check detects stale committed artifacts."""
import argparse
import copy
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parent.parent


def ts_type(schema, output=False):
    if isinstance(schema, dict):
        schema = {k: v for k, v in schema.items() if k not in ('title', 'description', 'default', 'examples', '$schema')}
    if schema is True or schema == {}:
        return 'Json'
    if schema is False:
        return 'never'
    supported = {'$ref', 'const', 'enum', 'anyOf', 'oneOf', 'allOf', 'type', 'items',
                 'minItems', 'maxItems', 'uniqueItems', 'properties', 'required',
                 'additionalProperties', 'format', 'minimum', 'maximum', 'minLength', 'maxLength'}
    unknown = set(schema) - supported
    if unknown:
        raise ValueError(f'Unsupported schema keywords: {sorted(unknown)}')
    if '$ref' in schema:
        ref = schema['$ref']
        if not ref.startswith('#/definitions/'):
            raise ValueError(f'Unsupported reference: {ref}')
        return ref.rsplit('/', 1)[1]
    if 'const' in schema:
        return json.dumps(schema['const'], ensure_ascii=False)
    if 'enum' in schema:
        return ' | '.join(json.dumps(v, ensure_ascii=False) for v in schema['enum'])
    for key, separator in [('anyOf', ' | '), ('oneOf', ' | '), ('allOf', ' & ')]:
        if key in schema:
            return '(' + separator.join(ts_type(v, output) for v in schema[key]) + ')'
    kind = schema.get('type')
    if isinstance(kind, list):
        return '(' + ' | '.join(ts_type(dict(schema, type=t), output) for t in kind) + ')'
    if kind in ('integer', 'number'):
        return 'number'
    if kind in ('string', 'boolean', 'null'):
        return kind
    if kind == 'array':
        items = schema.get('items', {})
        if isinstance(items, list):
            if schema.get('minItems') != len(items) or schema.get('maxItems') != len(items):
                raise ValueError('Only fixed tuples are supported')
            return '[' + ', '.join(ts_type(v, output) for v in items) + ']'
        if schema.get('minItems') == schema.get('maxItems') and isinstance(schema.get('minItems'), int):
            return '[' + ', '.join(ts_type(items, output) for _ in range(schema['minItems'])) + ']'
        return 'Array<' + ts_type(items, output) + '>'
    if kind == 'object':
        properties = schema.get('properties', {})
        additional = schema.get('additionalProperties', True)
        if not properties:
            return 'Record<string, ' + ts_type(additional, output) + '>'
        required = schema.get('required', [])
        # Unknown fields accepted by serde are discarded; do not expose them as known members.
        members = [json.dumps(k) + ('' if output or k in required else '?') + ': ' + ts_type(v, output) + ';'
                   for k, v in sorted(properties.items())]
        return '{ ' + ' '.join(members) + ' }'
    raise ValueError(f'Unsupported schema shape: {schema}')


def portable_schema(name, schema):
    # schemars 1 uses '#' for recursive roots. Publish the established named
    # definition reference so each schema also composes into the shared TS map.
    schema = copy.deepcopy(schema)
    root_reference = False
    def walk(value):
        nonlocal root_reference
        if isinstance(value, dict):
            if value.get('$ref') == '#':
                value['$ref'] = '#/definitions/' + name.replace('~', '~0').replace('/', '~1')
                root_reference = True
            # Traverse schema positions only; defaults/examples/const may contain
            # arbitrary user JSON whose $ref strings must remain untouched.
            for keyword in ('properties', 'patternProperties', 'definitions', '$defs', 'dependentSchemas'):
                for child in value.get(keyword, {}).values(): walk(child)
            for keyword in ('allOf', 'anyOf', 'oneOf', 'prefixItems'):
                for child in value.get(keyword, []): walk(child)
            for keyword in ('items', 'additionalProperties', 'additionalItems', 'contains',
                            'propertyNames', 'not', 'if', 'then', 'else'):
                child = value.get(keyword)
                if isinstance(child, list):
                    for item in child: walk(item)
                else:
                    walk(child)
    walk(schema)
    if root_reference:
        definition = {k: v for k, v in schema.items() if k not in ('$schema', 'definitions', 'title')}
        definitions = schema.setdefault('definitions', {})
        if name in definitions and definitions[name] != definition:
            raise ValueError(f'Conflicting recursive root: {name}')
        definitions[name] = definition
    return schema


def artifacts(schemas):
    definitions = {}
    files = {}
    for name, schema in sorted(schemas.items()):
        schema = portable_schema(name, schema)
        files[ROOT / 'bindings/schema' / (name + '.schema.json')] = json.dumps(schema, ensure_ascii=False, indent=2, sort_keys=True) + '\n'
        for key, definition in {**schema.get('definitions', {}), name: {k: v for k, v in schema.items() if k not in ('$schema', 'definitions', 'title')}}.items():
            if key in definitions and definitions[key] != definition:
                raise ValueError(f'Conflicting Rust schema name: {key}')
            definitions[key] = definition
    source = '// Generated by scripts/generate_contracts.py. Do not edit.\n'
    source += '// Deserialization shapes. Rust remains responsible for semantic validation.\n'
    source += '// Rust integers become number; callers must stay within the JS safe integer range.\n'
    source += 'export type Json = null | boolean | number | string | Json[] | { [key: string]: Json };\n'
    source += 'export type Id = string;\n'
    for output, filename in [(False, 'generated.ts'), (True, 'generated-output.ts')]:
        body = source
        if output:
            body = body.replace('Deserialization shapes.', 'Serialization shapes (selected Rust types emit every field).')
        for name, schema in sorted(definitions.items()):
            body += 'export type ' + name + ' = ' + ts_type(schema, output) + ';\n'
        files[ROOT / 'bindings/typescript' / filename] = body
    return files


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    result = subprocess.run(['cargo', 'run', '--locked', '--quiet', '-p', 'unge-contracts'], cwd=ROOT, text=True, capture_output=True)
    if result.returncode:
        raise SystemExit(result.stderr)
    files = artifacts(json.loads(result.stdout))
    port_result = subprocess.run(["cargo", "run", "--locked", "--quiet", "-p", "unge-contracts", "--", "--port-schemas"], cwd=ROOT, text=True, capture_output=True, check=True)
    files[ROOT / "bindings/schema/math-port-values.json"] = json.dumps(json.loads(port_result.stdout), ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    stale = []
    for path, source in files.items():
        if args.check:
            if not path.exists() or path.read_text() != source:
                stale.append(str(path.relative_to(ROOT)))
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(source)
    expected = set(files)
    obsolete = set((ROOT / 'bindings/schema').glob('*.schema.json')) - expected
    if obsolete:
        raise SystemExit('Obsolete schema files: ' + ', '.join(str(p) for p in sorted(obsolete)))
    if stale:
        raise SystemExit('Stale contracts; run python3 scripts/generate_contracts.py:\n' + '\n'.join(stale))
    print(f'{len(files)} contracts ' + ('checked' if args.check else 'generated'))


if __name__ == '__main__':
    main()

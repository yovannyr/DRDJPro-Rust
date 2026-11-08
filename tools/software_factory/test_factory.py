import copy
import json
import re
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import factory


class FactoryTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.registry = factory.extract_registry()
        cls.nodes = {x['id']: x for x in cls.registry['items']}

    def test_plan_coverage_and_first_slice(self):
        self.assertEqual(len(self.nodes), 233)
        self.assertEqual(sum(x['kind'] == 'package' for x in self.nodes.values()), 50)
        state = {'version': 1, 'slices': {}}
        self.assertEqual([x['id'] for x in self.registry['items'] if factory.ready(x, self.nodes, state)], ['G01'])
        self.assertFalse(factory.ready(self.nodes['P03a.1'], self.nodes, state))

    def test_packages_require_all_children_and_evidence(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            proof = root / 'proof.json'
            proof.write_text('{}', encoding='utf-8')
            state = {'version': 1, 'slices': {}}
            for child in self.nodes['P01a']['children']:
                state['slices'][child] = {'status': 'complete', 'evidence': 'proof.json', 'evidence_sha256': factory.digest(proof)}
            with patch.object(factory, 'ROOT', root):
                self.assertTrue(factory.complete('P01a', self.nodes, state))
                proof.write_text('changed', encoding='utf-8')
                self.assertFalse(factory.complete('P01a', self.nodes, state))

    def test_rejects_cycle_and_missing_dependency(self):
        data = copy.deepcopy(self.registry)
        data['items'][0]['dependencies'] = [data['items'][0]['id']]
        with self.assertRaisesRegex(ValueError, 'Zyklisch'):
            factory.validate(data)
        data['items'][0]['dependencies'] = ['G99']
        with self.assertRaisesRegex(ValueError, 'Unbekannte'):
            factory.validate(data)

    def test_all_spec_functional_requirements_are_assigned(self):
        assigned = {(x.get('spec'), req) for x in self.registry['items'] for req in x['requirements']}
        for path in (factory.ROOT / 'docs/specs').glob('*/spec.md'):
            number = int(path.parent.name.split('-')[0])
            for requirement in set(re.findall(r'FR-(?:\d{3}-)?\d{3}', path.read_text(encoding='utf-8'))):
                self.assertIn((number, requirement), assigned)
        self.assertEqual(sum(len(x['requirements']) for x in self.nodes.values() if x['id'].startswith('Q')), 200)

    def test_blocked_slice_is_not_ready(self):
        state = {'version': 1, 'slices': {'G01': {'status': 'blocked', 'reason': 'open'}}}
        self.assertFalse(factory.ready(self.nodes['G01'], self.nodes, state))
        self.assertFalse(factory.ready(self.nodes['G02'], self.nodes, state))

    def test_g01_cannot_finish_with_incomplete_attestation(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            evidence = root / 'evidence.json'
            evidence.write_text(json.dumps({'slice': 'G01', 'open_blockers': [], 'checks': [{'command': 'test', 'result': 'passed'}]}), encoding='utf-8')
            with patch.object(factory, 'ROOT', root), patch.object(factory, 'load_registry', return_value=self.registry), patch.object(factory, 'load_state', return_value={'version': 1, 'slices': {}}):
                self.assertEqual(factory.main(['finish', 'G01', '--evidence', 'evidence.json']), 1)

    def test_stale_plan_refuses_execution(self):
        with tempfile.TemporaryDirectory() as temp:
            fake_plan = Path(temp) / 'plan.md'
            fake_plan.write_text('changed', encoding='utf-8')
            fake_registry = Path(temp) / 'registry.json'
            fake_registry.write_text(__import__('json').dumps(self.registry), encoding='utf-8')
            with patch.object(factory, 'PLAN', fake_plan), patch.object(factory, 'REGISTRY', fake_registry):
                with self.assertRaisesRegex(ValueError, 'geändert'):
                    factory.load_registry()

    def test_source_digest_ignores_checkout_line_endings(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / 'plan.md'
            path.write_bytes(b'one\ntwo\n')
            expected = factory.source_digest(path)
            path.write_bytes(b'one\r\ntwo\r\n')
            self.assertEqual(factory.source_digest(path), expected)

    def test_pilot_preserves_existing_report(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / 'docs/specs').mkdir(parents=True)
            report = root / 'docs/specs/G01_FACTORY_PILOT.md'
            report.write_text('existing', encoding='utf-8')
            with patch.object(factory, 'ROOT', root), patch.object(factory, 'load_registry', return_value=self.registry), patch.object(factory, 'load_state', return_value={'version': 1, 'slices': {}}):
                self.assertEqual(factory.main(['pilot-g01']), 1)
            self.assertEqual(report.read_text(encoding='utf-8'), 'existing')


if __name__ == '__main__':
    unittest.main()

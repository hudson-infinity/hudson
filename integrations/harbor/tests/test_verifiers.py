"""Held-out checks reject wrong deliverables; no model or shell execution."""
import json
import os
from pathlib import Path
import runpy
import tempfile
import unittest
from unittest.mock import patch

TASKS = Path(__file__).parents[1] / "tasks"


class VerifierTest(unittest.TestCase):
    def verify(self, task, root):
        with patch.dict(os.environ, {"TASK_ROOT": str(root)}):
            runpy.run_path(str(TASKS / task / "tests/check.py"))

    def test_coding_checks_generalize_beyond_examples(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "intervals.py").write_text("def merge(intervals):\n result=[]\n for a,b in sorted(intervals):\n  if result and a<=result[-1][1]: result[-1][1]=max(result[-1][1],b)\n  else: result.append([a,b])\n return result\n")
            self.verify("coding-intervals", root)
            (root / "intervals.py").write_text("def merge(intervals): return sorted(intervals)\n")
            with self.assertRaises(AssertionError):
                self.verify("coding-intervals", root)

    def test_data_checks_exclusion_and_exact_cents(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            correct = {"net_by_region": {"East": 25505, "West": 23000}, "top_region": "East", "excluded_rows": 2}
            (root / "report.json").write_text(json.dumps(correct))
            self.verify("data-sales", root)
            correct["excluded_rows"] = 0
            (root / "report.json").write_text(json.dumps(correct))
            with self.assertRaises(AssertionError):
                self.verify("data-sales", root)

    def test_research_checks_citations_not_just_answer(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            correct = {"budget_usd": 4250000, "construction_started": False, "citations": {"budget": "2025-03-board.txt", "construction": "2025-04-status.txt"}}
            (root / "research.json").write_text(json.dumps(correct))
            self.verify("research-evidence", root)
            correct["citations"]["budget"] = "2025-02-commentary.txt"
            (root / "research.json").write_text(json.dumps(correct))
            with self.assertRaises(AssertionError):
                self.verify("research-evidence", root)

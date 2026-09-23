import unittest
from hudson_harbor.report import compare


class PricingTest(unittest.TestCase):
    def test_explicit_rates_and_incomplete_usage(self):
        trial = {"reward": 1, "elapsed_ms": 100, "budgets": {"calls": 2},
                 "usage": {"model_calls": 2, "reported_model_calls": 2,
                           "reported_input_tokens": 1000, "reported_output_tokens": 500}}
        jobs = {"task": [trial]}
        rates = {"baseline": {"input_per_million": 2, "output_per_million": 6},
                 "candidate": {"input_per_million": 1, "output_per_million": 2}}
        result = compare(jobs, jobs, rates)["tasks"][0]
        self.assertAlmostEqual(result["baseline"]["estimated_cost_usd"], 0.005)
        self.assertAlmostEqual(result["candidate"]["estimated_cost_usd"], 0.002)
        trial["usage"]["reported_model_calls"] = 1
        self.assertIsNone(compare(jobs, jobs, rates)["tasks"][0]["baseline"]["estimated_cost_usd"])
        rates["baseline"]["input_per_million"] = -1
        with self.assertRaises(ValueError):
            compare(jobs, jobs, rates)

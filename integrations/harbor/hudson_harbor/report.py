"""Compare independent Harbor verifier rewards and Hudson usage under equal budgets."""
import argparse
import json
from pathlib import Path
import statistics
import math


def load_job(path):
    trials = {}
    for result_path in Path(path).glob("*/result.json"):
        result = json.loads(result_path.read_text())
        task = result.get("task_name") or result.get("config", {}).get("task", {}).get("path")
        if not task:
            raise ValueError(f"missing task identity: {result_path}")
        key = str(task)
        context = result.get("agent_result") or {}
        if "metadata" not in context:
            context_path = result_path.parent / "agent" / "hudson-context.json"
            if context_path.exists():
                context = json.loads(context_path.read_text())
        hudson = (context.get("metadata") or {}).get("hudson", {})
        rewards = (result.get("verifier_result") or {}).get("rewards") or {}
        if not hudson.get("budgets"):
            raise ValueError(f"missing Hudson budget record: {result_path}")
        trial = {"reward": rewards.get("reward"), "status": hudson.get("status"),
                 "elapsed_ms": hudson.get("elapsed_ms"), "usage": hudson.get("usage", {}),
                 "budgets": hudson["budgets"]}
        trials.setdefault(key, []).append(trial)
    if not trials:
        raise ValueError("no Harbor trial results found")
    return trials


def compare(baseline, candidate, pricing=None):
    if pricing is not None:
        for side in ("baseline", "candidate"):
            rates = pricing[side]
            for key in ("input_per_million", "output_per_million"):
                if type(rates[key]) not in (int, float) or not math.isfinite(rates[key]) or rates[key] < 0:
                    raise ValueError("pricing must contain finite nonnegative rates")
    if baseline.keys() != candidate.keys():
        raise ValueError("comparison requires the same task set")
    rows = []
    for name in sorted(baseline):
        a, b = baseline[name], candidate[name]
        if len(a) != len(b):
            raise ValueError("comparison requires equal repetitions")
        if any(x["budgets"] != a[0]["budgets"] for x in a + b):
            raise ValueError("comparison requires identical budgets")
        def summary(trials, side):
            rewards = [x["reward"] for x in trials]
            reported = sum(x["usage"].get("reported_model_calls", 0) for x in trials)
            cost = None
            if pricing is not None and all(x["usage"].get("reported_model_calls", 0) == x["usage"].get("model_calls", 0)
                                          and x["usage"].get("model_calls", 0) > 0 for x in trials):
                rates = pricing[side]
                cost = sum(x["usage"].get("reported_input_tokens", 0) * rates["input_per_million"]
                           + x["usage"].get("reported_output_tokens", 0) * rates["output_per_million"]
                           for x in trials) / 1_000_000
            return {"trials": len(trials), "verified_successes": sum(x == 1 for x in rewards),
                    "missing_rewards": sum(x is None for x in rewards),
                    "mean_elapsed_ms": statistics.mean(x["elapsed_ms"] for x in trials),
                    "model_calls": sum(x["usage"].get("model_calls", 0) for x in trials),
                    "reported_model_calls": reported,
                    "reported_input_tokens": sum(x["usage"].get("reported_input_tokens", 0) for x in trials) if reported else None,
                    "reported_output_tokens": sum(x["usage"].get("reported_output_tokens", 0) for x in trials) if reported else None,
                    "estimated_cost_usd": cost}
        rows.append({"task": name, "baseline": summary(a, "baseline"), "candidate": summary(b, "candidate")})
    return {"tasks": rows, "note": "Verifier rewards measure correctness; costs use supplied rates only when every model call reports usage. Unknown cost is null."}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("baseline")
    parser.add_argument("candidate")
    parser.add_argument("--pricing", type=Path, help="JSON baseline/candidate input_per_million and output_per_million USD rates")
    args = parser.parse_args()
    print(json.dumps(compare(load_job(args.baseline), load_job(args.candidate), json.loads(args.pricing.read_text()) if args.pricing else None), indent=2))

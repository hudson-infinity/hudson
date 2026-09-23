import json,os
from pathlib import Path
v=json.loads((Path(os.environ.get("TASK_ROOT","/app"))/"research.json").read_text())
assert v.get("budget_usd")==4250000
assert v.get("construction_started") is False
assert v.get("citations")=={"budget":"2025-03-board.txt","construction":"2025-04-status.txt"}

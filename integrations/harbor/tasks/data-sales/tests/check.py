import json,os
from pathlib import Path
v=json.loads((Path(os.environ.get("TASK_ROOT","/app"))/"report.json").read_text())
assert v=={"net_by_region":{"East":25505,"West":23000},"top_region":"East","excluded_rows":2},v

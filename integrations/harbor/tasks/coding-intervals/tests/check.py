import importlib.util, os, random
from pathlib import Path
root=Path(os.environ.get("TASK_ROOT","/app"))
spec=importlib.util.spec_from_file_location("intervals",root/"intervals.py")
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
def reference(items):
 out=[]
 for a,b in sorted(items):
  if out and a<=out[-1][1]:out[-1][1]=max(out[-1][1],b)
  else:out.append([a,b])
 return out
rng=random.Random(97127)
cases=[[],[[1,1]],[[4,5],[1,4]],[[8,9],[-4,-1],[-2,3]],[[1,10],[2,3]]]
for _ in range(100):
 cases.append([sorted([rng.randint(-100,100),rng.randint(-100,100)]) for _ in range(rng.randrange(20))])
for case in cases:
 original=[x[:] for x in case]
 assert m.merge(case)==reference(original),original
 assert case==original,"input mutated"

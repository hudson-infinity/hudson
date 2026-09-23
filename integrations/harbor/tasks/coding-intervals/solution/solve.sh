#!/bin/sh
cat > /app/intervals.py <<'PYCODE'
def merge(intervals):
 result=[]
 for a,b in sorted(intervals):
  if result and a<=result[-1][1]: result[-1][1]=max(result[-1][1],b)
  else: result.append([a,b])
 return result
PYCODE

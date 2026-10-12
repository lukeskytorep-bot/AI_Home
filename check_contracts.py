"""Run the accepted P2 1.1 reference suite; refuse drift of live contracts."""
from pathlib import Path
import runpy
root=Path(__file__).resolve().parent
snapshot=root/'snapshots/p2-v1.1'
for name in ['domain.schema.json','ipc.catalog.json','defaults.json','fixtures.json']:
    if (root/'contracts'/name).read_bytes()!=(snapshot/'contracts'/name).read_bytes():
        raise SystemExit('P2 contract drift: '+name)
runpy.run_path(str(snapshot/'check_contracts.py'),run_name='__main__')

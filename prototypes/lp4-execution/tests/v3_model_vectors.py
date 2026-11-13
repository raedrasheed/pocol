#!/usr/bin/env python3
"""The earn-in model (earnin_model.py) at the v3 probation pool of V3-14 / O-14=b: P_MAX 64, T_PROB 86,400
blocks (30 days at 30 s), with M_MAX 256 (V3-07) so the 65th probation seat is refused by P_MAX, not by M_max.

Writes tests/v3/earnin-vectors-p64.json, which tests/v3_contracts.rs replays on the v3 programs with those
values in CP schema v2. The model's rules are unchanged; only its three constants are set. Run with --check to
compare against the committed file, or --write to regenerate it.
"""
import json, sys
from pathlib import Path
sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
import earnin_model as model

P_MAX, T_PROB, M_MAX = 64, 86_400, 256

def vectors():
    saved = (model.P_MAX, model.T_PROB, model.M_MAX)
    model.P_MAX, model.T_PROB, model.M_MAX = P_MAX, T_PROB, M_MAX
    try:
        v = model.vectors()
    finally:
        model.P_MAX, model.T_PROB, model.M_MAX = saved
    v['params'] = {'P_MAX': P_MAX, 'T_PROB': T_PROB, 'M_MAX': M_MAX}
    return v

if __name__ == '__main__':
    path = Path(__file__).with_name('v3') / 'earnin-vectors-p64.json'
    data = json.dumps(vectors(), indent=1, sort_keys=True) + '\n'
    if sys.argv[1:] == ['--write']:
        path.write_text(data)
        print('wrote', path)
    else:
        assert path.read_text() == data, 'earnin-vectors-p64.json differs from the model; run --write and review'
        print('PASS v3 earn-in model (P_MAX 64, T_PROB 86400) matches', path.name)

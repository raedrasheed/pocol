"""Independent optional Blake2 native/Python differential, reproducible public inputs."""
import random
from importlib.metadata import version
from eth._utils.blake2.compression import blake2b_compress
from eth.precompiles.blake2 import blake2b_compress as active
from blake2b import compress

def main():
    assert version('blake2b-py')=='0.3.2'
    assert active is compress, 'independent engine must actually load the pinned native backend'
    r=random.Random(777902);count=0
    for rounds in [0,1,2,9,10,12,13,31,100,255]:
        for flag in [False,True]:
            for _ in range(8):
                args=rounds,tuple(r.getrandbits(64) for _ in range(8)),tuple(r.getrandbits(64) for _ in range(16)),tuple(r.getrandbits(64) for _ in range(2)),flag
                assert compress(*args)==blake2b_compress(*args);count+=1
    print(f'PASS {count} independent native/pure-Python Blake2 comparisons; zero/1/10/12 rounds, modulo schedule, counters and final flags; full P2 NOT PASSED')
if __name__=='__main__':main()

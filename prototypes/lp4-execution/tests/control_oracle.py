"""Independent restricted-opcode control oracle, NOT a full EVM/GST oracle.
Only the exact listed opcodes/initial conditions are supported; fail on any other.
"""
import json, sys

def run(code):
    stack, memory, slots, pc, gas = [], bytearray(), {}, 0, 21000
    def grow(end):
        nonlocal gas
        words=(end+31)//32
        old=len(memory)//32
        gas += 3*words+words*words//512-(3*old+old*old//512)
        memory.extend(bytes(32*words-len(memory)))
    while pc < len(code):
        op=code[pc]; pc+=1
        if op == 0x60:
            stack.append(code[pc]); pc+=1; gas+=3
        elif op == 0x46:
            stack.append(777902); gas+=2
        elif op == 0x52:
            offset,value=stack.pop(),stack.pop(); grow(offset+32)
            memory[offset:offset+32]=value.to_bytes(32,'big'); gas+=3
        elif op == 0x55:
            key,value=stack.pop(),stack.pop()
            assert key not in slots and value != 0, 'restricted zero-to-nonzero cold SSTORE only'
            slots[key]=value; gas+=22100
        elif op in (0xf3,0xfd):
            offset,size=stack.pop(),stack.pop()
            if size: grow(offset+size)
            return dict(success=op==0xf3,gas=gas,output=list(memory[offset:offset+size]),slot0=str(slots.get(0,0) if op==0xf3 else 0))
        else: raise AssertionError(f'unsupported opcode {op:x}')
    raise AssertionError('missing termination')

rows=json.load(open(sys.argv[1]))
assert len(rows)==3
for row in rows:
    expected=run(row['code'])
    assert {k:row[k] for k in expected} == expected, (row,expected)
print('PASS: 3 restricted control executions compared independently; full GST/T2 NOT PASSED')

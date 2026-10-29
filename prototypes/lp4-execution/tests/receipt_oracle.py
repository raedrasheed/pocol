"""Independent RLP, bloom and MPT over root-only fixtures, not accepted blocks."""
import json,sys,rlp
from eth_bloom import BloomFilter
from trie import HexaryTrie
from eth_hash.auto import keccak
rows=json.load(open(sys.argv[1]));results=json.load(open(sys.argv[2]))
assert [r['count'] for r in results]==[0,1,2,18,130]
for result in results:
    txs=HexaryTrie(db={});receipts=HexaryTrie(db={});cumulative=0;block_bloom=0;encoded=[]
    for i in range(result['count']):
        row=rows[i%len(rows)];txs[rlp.encode(i)]=bytes.fromhex(row['raw']);cumulative+=row['gasUsed'];bloom=BloomFilter();logs=[]
        for log in row['logs']:
            address=bytes.fromhex(log['address'][2:]);topics=[bytes.fromhex(t[2:]) for t in log['topics']];data=bytes.fromhex(log['data'][2:]);logs.append([address,topics,data]);bloom.add(address)
            for t in topics:bloom.add(t)
        block_bloom|=int(bloom);kind=row['decoded']['kind'];payload=rlp.encode([int(row['success']),cumulative,int(bloom).to_bytes(256,'big'),logs]);receipt=(bytes([kind]) if kind else b'')+payload
        receipts[rlp.encode(i)]=receipt;encoded.append(list(receipt))
    assert '0x'+txs.root_hash.hex()==result['txRoot'] and '0x'+receipts.root_hash.hex()==result['receiptsRoot'],('root',result['count'])
    assert cumulative==result['cumulativeGas'] and list(block_bloom.to_bytes(256,'big'))==result['bloom'] and encoded==result['receipts'],('encoding',result['count'])
print('PASS5 independent ordered tx/receipt MPT and cumulative-gas/bloom encoding fixtures; repeated envelopes are NOT valid blocks')

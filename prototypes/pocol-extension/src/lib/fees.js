// Sites W4: the publisher's fee schedule. `nextBaseFee` is the exact London arithmetic of the node
// (lp4-execution/src/fees.rs next_base_fee, with the v3 floor baseFeeMin); tests/publisher_test.mjs checks it against
// the fees.rs vectors (lp4-execution/tests/data/london-fees.json) and a reference recurrence.
//
// A publication is a known sequence of transactions (each chunk, the Website deploy, createVersion, publish,
// setCurrent). `feeSchedule` simulates the base fee over that sequence, one publication transaction per block and the
// blocks otherwise empty (economy mode: an empty block after each one), each transaction counted at its estimated
// gasUsed (`gasUsed`; a chunk store at chunkGasUsed, any other transaction at its declared gas, an upper bound). Each transaction's fee cap is the same
// rule on its projected base fee: ceil(base * (1 + margin)) + tip, with margin 12.5 % (one block of maximum rise).
// The user approves one budget (the sum of gas * cap) and one per-gas ceiling (twice the largest cap, never above
// 200 gwei). At signing the publisher applies the same rule to the live base fee and signs with the larger of the two
// caps, so on a chain that follows the projection it signs exactly the approved caps and never pauses.
export const GWEI = 1000000000n;
export const GAS_LIMIT = 8000000n;            // CP GAS_LIMIT (pocold rpc.rs, eth_feeHistory gasUsedRatio)
export const BASE_FEE_FLOOR = 10000000n;      // v3 baseFeeMin, 10^7 wei (design row 55)
export const NODE_TIP = GWEI;                 // eth_maxPriorityFeePerGas of the node (rpc.rs)
export const TIP_CAP = 2n * GWEI;             // the publisher never takes a larger tip from the node or the user
export const ABSOLUTE_CEILING = 200n * GWEI;  // no approval may exceed 200 gwei per gas
export const MARGIN_PER_MILLE = 125n;         // 12.5 %
/** The gas a ChunkFactory store(bytes) of `size` bytes uses at most: 21000 + 32000 (CREATE2), the code deposit of
 *  200 per byte (with the 0x00 prefix), calldata at 16 per byte and the hashing, copy and memory words, with headroom.
 *  A full chunk (24,575 B) is 5,427,350 here against the measured ~5.395M; its declared gas is 200000 + 260 * size. */
export const chunkGasUsed = (size) => 70000n + 218n * BigInt(size);
const uint = (v, name) => { let n; try { n = BigInt(v); } catch { throw new Error('Invalid ' + name); } if (n < 0n || n >= 1n << 256n) throw new Error('Invalid ' + name); return n; };
const ceilDiv = (a, b) => (a + b - 1n) / b;

/** fees.rs next_base_fee: London arithmetic with the floor. Throws 'ParentGas' or 'Overflow' as the node errs. */
export function nextBaseFee(parent, gasUsed, gasLimit = GAS_LIMIT, floor = 0n) {
  parent = BigInt(parent); gasUsed = BigInt(gasUsed); gasLimit = BigInt(gasLimit); floor = BigInt(floor);
  const target = gasLimit / 2n;
  if (target === 0n || gasUsed > gasLimit) throw new Error('ParentGas');
  if (gasUsed === target) return parent > floor ? parent : floor;
  const distance = gasUsed > target ? gasUsed - target : target - gasUsed;
  const delta = parent * distance / target / 8n;
  const fee = gasUsed > target ? parent + (delta > 1n ? delta : 1n) : parent - delta;
  if (fee >= 1n << 256n) throw new Error('Overflow');
  return fee > floor ? fee : floor;
}
/** The fee cap the publisher signs for a block whose base fee is `base`. */
export function capFor(base, tip, marginPerMille = MARGIN_PER_MILLE) {
  return ceilDiv(BigInt(base) * (1000n + BigInt(marginPerMille)), 1000n) + BigInt(tip);
}
/**
 * The schedule for `txs` ([{id, gas}] in signing order) from the next block's base fee `baseNext`.
 * Returns {schedule:[{id,gas,base,maxFee}], budgetWei, ceilingWei, expectedWei, finalBaseWei} as decimal strings.
 */
export function feeSchedule({txs, baseNext, tip = NODE_TIP, economy = false, gasLimit = GAS_LIMIT, floor = BASE_FEE_FLOOR, marginPerMille = MARGIN_PER_MILLE}) {
  if (!Array.isArray(txs) || !txs.length) throw new Error('A fee schedule needs at least one transaction');
  let base = uint(baseNext, 'base fee'); tip = uint(tip, 'tip');
  if (tip > TIP_CAP) throw new Error('The tip may not exceed 2 gwei');
  const limit = uint(gasLimit, 'gas limit'), low = uint(floor, 'base fee floor');
  if (base < low) base = low;
  let budget = 0n, expected = 0n, top = 0n; const schedule = [];
  for (const tx of txs) {
    const gas = uint(tx.gas, 'gas'); if (gas === 0n || gas > limit) throw new Error('Invalid transaction gas');
    const used = tx.gasUsed === undefined ? gas : uint(tx.gasUsed, 'gas used'); if (used > gas) throw new Error('Invalid transaction gas');
    const maxFee = capFor(base, tip, marginPerMille);
    schedule.push({id: tx.id, gas: gas.toString(), gasUsed: used.toString(), base: base.toString(), maxFee: maxFee.toString()});
    budget += gas * maxFee; expected += used * (base + tip); if (maxFee > top) top = maxFee;
    base = nextBaseFee(base, used, limit, low);
    if (economy) base = nextBaseFee(base, 0n, limit, low);
  }
  if (top > ABSOLUTE_CEILING) throw new Error('The projected fee exceeds the absolute ceiling of 200 gwei per gas');
  const ceiling = top * 2n > ABSOLUTE_CEILING ? ABSOLUTE_CEILING : top * 2n;
  return {schedule, budgetWei: budget.toString(), ceilingWei: ceiling.toString(), expectedWei: expected.toString(), finalBaseWei: base.toString()};
}
const quantity = (v) => { if (typeof v !== 'string' || !/^0x[0-9a-fA-F]{1,64}$/.test(v)) throw new Error('Invalid fee quantity'); return BigInt(v); };
/**
 * The next block's base fee and the tip, read from the node: eth_gasPrice minus the node's tip, cross-checked with
 * eth_feeHistory (its last base fee is the next block's) and the London step from the latest block. The largest
 * reading is used. The tip is eth_maxPriorityFeePerGas capped at 2 gwei (1 gwei when the node does not answer).
 */
export async function readNetworkFees(rpc, {floor = BASE_FEE_FLOOR} = {}) {
  let nodeTip = NODE_TIP;
  try { nodeTip = quantity(await rpc('eth_maxPriorityFeePerGas', [])); } catch (e) { if (!e?.rpc && !e?.transport && !/fee quantity/.test(e?.message || '')) throw e; }
  if (nodeTip > TIP_CAP) nodeTip = TIP_CAP;
  const readings = [];
  const attempt = async (f) => { try { readings.push(await f()); } catch (e) { if (!e?.rpc && !e?.transport && !/fee quantity|baseFee/.test(e?.message || '')) throw e; } };
  await attempt(async () => { const p = quantity(await rpc('eth_gasPrice', [])); return p > nodeTip ? p - nodeTip : 0n; });
  await attempt(async () => { const h = await rpc('eth_feeHistory', ['0x1', 'latest', []]); const fees = h?.baseFeePerGas; if (!Array.isArray(fees) || !fees.length) throw new Error('baseFee history'); return quantity(fees[fees.length - 1]); });
  await attempt(async () => {
    const b = await rpc('eth_getBlockByNumber', ['latest', false]);
    if (!b || b.baseFeePerGas === undefined) throw new Error('baseFee missing');
    return nextBaseFee(quantity(b.baseFeePerGas), quantity(b.gasUsed), quantity(b.gasLimit), floor);
  });
  if (!readings.length) throw new Error('The network did not report its base fee');
  let baseNext = readings.reduce((a, b) => (a > b ? a : b));
  if (baseNext < BigInt(floor)) baseNext = BigInt(floor);
  return {baseNext: baseNext.toString(), tip: nodeTip.toString(), readings: readings.map(String)};
}

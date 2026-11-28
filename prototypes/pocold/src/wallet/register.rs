//! `pocol register status | quote | confirm` (Stage 4 PR A1): registration for a Linux server miner.
//!
//! `pocol start` (and the Linux service) always mines with its home's identity, and the node mines as soon as that
//! identity holds an eligible registry record; it has no registration step of its own, and the wallet engine cannot open
//! a home the running node holds. This command registers the identity beside the running node: it reads the home
//! without its lock ([`HomeReader`]), asks the node's local RPC for the chain facts, and signs the same transaction the
//! wallets' **Review mining registration** signs (`register(address)` below a full set, `registerReplacing(reward,
//! victim)` at a full set, with the wallets' gas reservations and fee rule). Nothing is signed or sent without
//! `confirm` and a typed `yes`; `status` and `quote` read only.
//!
//! The facts come from the node at `--rpc`, which must be a loopback address (this host's own node): its chain id and
//! genesis hash must be the home's network file's, and the consensus parameters (B_reg, M_max, Theta_inact, U) are read
//! from that file, not from the node. Everything else (balance, nonce, base fee, seats, the replaced member) is the
//! node's answer, trusted as this host's own node is: a process of another local user listening on that port while
//! the node is down could answer instead, and would receive the signed registration (only that transaction, after the
//! operator reviewed its quote and typed yes). The signed transaction always names this network's chain id, the
//! registry, the quoted bond and gas, and a maximum fee the quote shows. The registry judges the transaction again when it is included; a lost race (the
//! per-block registration limit, or a replaced member that mined or was taken first) reverts it and spends only the fee.
use super::{amount, coins, register_calldata, replacing_calldata, ReplacePlan, EARNIN_REGISTER_GAS, REPLACE_GAS};
use crate::{launcher::HomeReader, site::Rpc, types};
use pocol_lp4_execution::{apply, txsign::Tx};
use revm::primitives::{keccak256, Address, U256};
use serde_json::{json, Value};
use std::{
    net::SocketAddr,
    time::{Duration, Instant},
};

/// The node RPC the command asks when `--rpc` is not given (`pocol start`'s default).
pub const DEFAULT_RPC: &str = "127.0.0.1:8545";
/// The wallets' fee rule: a 1 gwei tip and a maximum fee of twice the next base fee plus the tip.
const GWEI: u64 = 1_000_000_000;
/// The wallets' head age limit: an older head means the node is behind, so its facts are stale.
const HEAD_MAX_AGE_S: u64 = 180;
/// A confirmation must be typed within this long of the quote it signs (the wallets' quote lifetime).
pub const QUOTE_LIFETIME: Duration = Duration::from_secs(60);
/// How long `confirm` waits for the receipt before it reports the transaction as sent and not yet included.
const RECEIPT_WAIT: Duration = Duration::from_secs(180);

/// What the home's network file fixes: the chain, and the registry parameters the quote states.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Network {
    pub chain_id: u64,
    pub genesis_hash: String,
    pub b_reg: U256,
    pub m_max: u64,
    pub theta_inact: u64,
    pub u: u64,
}
impl Network {
    /// From the bytes of `genesis.spec.hex`.
    pub fn from_spec(bytes: &[u8]) -> Result<Self, String> {
        let spec = crate::genesis_spec::decode(bytes).map_err(|e| format!("genesis specification: {e:?}"))?;
        let cp = |name: &str| spec.cp_value(name).map(|v| U256::from_be_bytes(v.to_be_bytes())).ok_or_else(|| format!("genesis specification has no {name}"));
        let small = |name: &str| cp(name).and_then(|v| u64::try_from(v).map_err(|_| format!("{name} is out of range")));
        Ok(Self {
            chain_id: spec.chain_id,
            genesis_hash: types::hex(&crate::genesis_spec::genesis_hash(bytes)),
            b_reg: cp("B_reg")?,
            m_max: small("M_max")?,
            theta_inact: small("Theta_inact")?,
            u: small("U")?,
        })
    }
}

/// The identity's registry record, decoded from the registry's `recordWords(id)` view as `apply::miner_record` decodes
/// the same four words from storage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub status: u8,
    pub tomb: bool,
    pub bond_state: u8,
    pub last_share: u64,
    pub registered_at: u64,
    pub reward: Address,
    pub bond: U256,
    pub exit_height: u64,
    pub due_height: u64,
}
fn bits(w: U256, at: usize, n: usize) -> u64 {
    let v: U256 = (w >> at) & ((U256::from(1u8) << n) - U256::from(1u8));
    v.to::<u64>()
}
impl Record {
    /// None for an absent record (word 0 is zero: the registry's `enroll` takes such an identity).
    pub fn decode(out: &[u8]) -> Result<Option<Self>, String> {
        if out.len() != 128 {
            return Err("The registry answered recordWords with an unexpected length".into());
        }
        let word = |i: usize| U256::from_be_slice(&out[32 * i..32 * (i + 1)]);
        let (w0, w1, w2, w3) = (word(0), word(1), word(2), word(3));
        if w0.is_zero() {
            return Ok(None);
        }
        Ok(Some(Self {
            status: bits(w0, 0, 8) as u8,
            tomb: bits(w0, 8, 8) != 0,
            bond_state: bits(w0, 16, 8) as u8,
            last_share: bits(w0, 88, 64),
            registered_at: bits(w0, 152, 64),
            reward: Address::from_word(w1.into()),
            bond: w2,
            exit_height: bits(w3, 0, 64),
            due_height: bits(w3, 64, 64),
        }))
    }
    /// A HELD or ACCRUING active record mines (the wallets' `miningEligible`).
    pub fn eligible(&self) -> bool {
        self.status == 1 && !self.tomb && matches!(self.bond_state, 1 | 4)
    }
    pub fn json(&self) -> Value {
        json!({"status":self.status,"tomb":self.tomb,"bondState":self.bond_state,"bondStateName":match self.bond_state{1=>"HELD",4=>"ACCRUING",_=>"OTHER"},
            "bond":coins(self.bond),"reward":format!("{:#x}",self.reward),"lastShare":self.last_share,"registeredAt":self.registered_at,
            "exitHeight":self.exit_height,"dueHeight":self.due_height})
    }
}

/// The seats at the next block (`pocol_getRemovable`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Seats {
    pub active: u64,
    pub seats: u64,
    pub full: bool,
    pub removable: Vec<Address>,
    pub pending_victims: Vec<Address>,
}

/// Everything the node reports that the decision needs; gathered over RPC by [`gather`], or built by tests.
#[derive(Clone, Debug)]
pub struct Facts {
    pub chain_id: u64,
    pub genesis_hash: String,
    pub peers: u64,
    pub head_height: u64,
    pub head_ts: u64,
    pub now: u64,
    pub record: Option<Record>,
    pub balance: U256,
    pub nonce_latest: u64,
    pub nonce_pending: u64,
    pub base_next: U256,
    pub seats: Seats,
    /// Whether the reward address (when it is not the identity itself) is durable for the registry: an account that
    /// has sent a transaction or holds code (`Sys.pay(arg, 0)`, lp4-execution pay.rs `durable`), judged at the head.
    pub reward_durable: bool,
}

/// What `confirm` signs, and what `quote` shows.
pub struct Plan {
    pub me: Address,
    pub reward: Address,
    pub tx: Tx,
    pub replacing: Option<ReplacePlan>,
    pub b_reg: U256,
}
impl std::fmt::Debug for Plan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "Plan({})", self.json()) }
}
impl Plan {
    pub fn max_fee_total(&self) -> U256 {
        self.tx.max_fee * U256::from(self.tx.gas)
    }
    pub fn json(&self) -> Value {
        let cost = self.max_fee_total();
        let value = self.tx.value;
        let mut q = json!({"type":"quote","kind":"registration","method":if self.replacing.is_some(){"registerReplacing"}else{"register"},
            "from":format!("{:#x}",self.me),"to":format!("{:#x}",apply::program_address(0)),"reward":format!("{:#x}",self.reward),
            "bond":coins(value),"bondTarget":coins(self.b_reg),"probation":value<self.b_reg,"maximumFee":coins(cost),"maximumTotal":coins(value+cost),
            "gas":self.tx.gas,"maxFeePerGas":self.tx.max_fee.to_string(),"maxPriorityFeePerGas":self.tx.max_priority.to_string(),
            "nonce":self.tx.nonce,"chainId":self.tx.chain_id,"expiresInSeconds":QUOTE_LIFETIME.as_secs()});
        match &self.replacing {
            Some(plan) => {
                if let Value::Object(fields) = plan.describe() {
                    for (k, v) in fields {
                        q[k] = v;
                    }
                }
            }
            None => {
                q["steps"] = json!([
                    format!("Registers {:#x} as a miner with a {} PCOL bond{}.", self.me, coins(value),
                        if value < self.b_reg { format!(", filled from its mining rewards up to {} PCOL (accruing)", coins(self.b_reg)) } else { String::new() }),
                    if self.reward == self.me { "Rewards are paid to the identity itself.".to_owned() } else { format!("Rewards are paid to {:#x}.", self.reward) },
                    "Once the transaction is included the node already running with this identity mines with it; no restart is needed.".to_owned(),
                ]);
                q["note"] = json!("Checked against the current head. The registry judges the registration again when it is included; if the per-block registration limit is reached or every seat is taken first, the transaction reverts and only the fee is spent.");
            }
        }
        q
    }
}

/// A reward address `--reward-address` names: the full `0x` form, and not zero, a precompile or a reserved system
/// address (the wallets' recipient rule).
pub fn parse_reward(text: &str) -> Result<Address, String> {
    if text.len() != 42 || !text.starts_with("0x") || !text[2..].bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("--reward-address: enter the full 0x address (42 characters)".into());
    }
    let a: Address = text.parse().map_err(|_| "--reward-address: invalid address")?;
    let numeric = U256::from_be_slice(a.as_slice());
    if a.is_zero() || numeric <= U256::from(0xffffu64) || (U256::from(0xc0c001u64)..=U256::from(0xc0c0ffu64)).contains(&numeric) {
        return Err("--reward-address cannot be zero, a precompile or a reserved system address".into());
    }
    Ok(a)
}
/// The bond `--bond` names: none or 0 for 0 (accruing), otherwise a decimal PCOL amount.
pub fn parse_bond(text: Option<&str>) -> Result<U256, String> {
    match text {
        None | Some("0") => Ok(U256::ZERO),
        Some(t) => amount(t).map_err(|e| format!("--bond: {e}")),
    }
}
/// `--rpc`: this host's own node (a loopback address), whose answers the command trusts for the chain facts.
pub fn parse_rpc(text: Option<&str>) -> Result<SocketAddr, String> {
    let text = text.unwrap_or(DEFAULT_RPC);
    let addr: SocketAddr = text.parse().map_err(|_| format!("--rpc {text}: expected IP:PORT"))?;
    if !addr.ip().is_loopback() || addr.port() == 0 {
        return Err("--rpc must be this host's own node (a loopback address such as 127.0.0.1:8545)".into());
    }
    Ok(addr)
}

/// The registration `me` would sign with `bond` and `reward`, from `facts`; every refusal is stated before anything is
/// signed. `pick(n)` draws the replaced member among n candidates; `judge(victim)` asks the registry's removalStatus
/// view (0 removable).
pub fn decide(net: &Network, facts: &Facts, me: Address, reward: Address, bond: U256, pick: impl FnOnce(usize) -> usize, judge: impl FnOnce(Address) -> Result<u64, String>) -> Result<Plan, String> {
    if facts.chain_id != net.chain_id || !facts.genesis_hash.eq_ignore_ascii_case(&net.genesis_hash) {
        return Err(format!("The node runs another network (chain {} genesis {}) than this home's network file (chain {} genesis {}); nothing was signed", facts.chain_id, facts.genesis_hash, net.chain_id, net.genesis_hash));
    }
    if let Some(r) = &facts.record {
        return Err(format!("This identity already has a registry record (status {}, bond state {}); registration is never repeated. See pocol register status.", r.status, r.bond_state));
    }
    if bond > net.b_reg {
        return Err(format!("The bond is at most {} PCOL", coins(net.b_reg)));
    }
    if reward != me && !facts.reward_durable {
        return Err(format!("--reward-address {reward:#x} cannot receive registry rewards yet: the registry accepts a reward account other than the identity only once it has sent a transaction (or holds code), so this registration would revert. Send one transaction from it first, or leave --reward-address out."));
    }
    if facts.peers == 0 {
        return Err("The node has no peers. Connect it to the network before registering.".into());
    }
    if facts.head_ts.saturating_add(HEAD_MAX_AGE_S) < facts.now {
        return Err("The node's chain is behind. Wait for synchronization, then try again.".into());
    }
    if facts.nonce_pending != facts.nonce_latest {
        return Err(format!("This identity has {} transaction(s) waiting in the node's pool. Wait until they are included, then try again.", facts.nonce_pending.saturating_sub(facts.nonce_latest)));
    }
    let s = &facts.seats;
    let (to_data, gas, replacing) = if s.full {
        if s.removable.is_empty() {
            return Err(format!("All {} seats are taken and every member has mined or joined within the last {} blocks, so none can be replaced yet. Try again later.", s.seats, net.theta_inact));
        }
        let list: Vec<Address> = s.removable.iter().copied().filter(|a| !s.pending_victims.contains(a)).collect();
        if list.is_empty() {
            return Err("Every removable member is already named by a pending replacement. Try again after the next block.".into());
        }
        let victim = list[pick(list.len()) % list.len()];
        let code = judge(victim)?;
        if code != 0 {
            return Err(format!("The registry refused the replacement (code {code}). Try again."));
        }
        let plan = ReplacePlan { victim, value: bond, full: net.b_reg, candidates: list.len(), theta_inact: net.theta_inact, u: net.u, m_max: net.m_max };
        (replacing_calldata(reward, victim), REPLACE_GAS, Some(plan))
    } else {
        (register_calldata(reward), EARNIN_REGISTER_GAS, None)
    };
    let priority = U256::from(GWEI);
    let max_fee = facts.base_next.checked_mul(U256::from(2)).and_then(|b| b.checked_add(priority)).ok_or("Fee overflow")?;
    let cost = max_fee.checked_mul(U256::from(gas)).ok_or("Fee overflow")?;
    let total = bond.checked_add(cost).ok_or("Total overflow")?;
    if total > facts.balance {
        return Err(format!("Insufficient available balance for the bond and maximum fee: needs {} PCOL, holds {} PCOL. Fund {:#x} first.", coins(total), coins(facts.balance), me));
    }
    let tx = Tx { chain_id: net.chain_id, nonce: facts.nonce_latest, max_priority: priority, max_fee, gas, to: Some(apply::program_address(0)), value: bond, data: to_data };
    Ok(Plan { me, reward, tx, replacing, b_reg: net.b_reg })
}

fn quantity(v: &Value, what: &str) -> Result<U256, String> {
    let s = v.as_str().ok_or_else(|| format!("{what}: not a quantity"))?;
    U256::from_str_radix(s.trim_start_matches("0x"), 16).map_err(|e| format!("{what}: {e}"))
}
fn small(v: &Value, what: &str) -> Result<u64, String> {
    u64::try_from(quantity(v, what)?).map_err(|_| format!("{what}: out of range"))
}
fn addresses(v: &Value, what: &str) -> Result<Vec<Address>, String> {
    v.as_array().ok_or_else(|| format!("{what}: not a list"))?.iter().map(|a| a.as_str().and_then(|s| s.parse().ok()).ok_or_else(|| format!("{what}: invalid address"))).collect()
}
fn call(rpc: &Rpc, from: Address, to: Address, data: Vec<u8>) -> Result<Vec<u8>, String> {
    let out = rpc.call("eth_call", json!([{"from":format!("{from:#x}"),"to":format!("{to:#x}"),"data":types::hex(&data)},"latest"]))?;
    types::unhex(out.as_str().unwrap_or("0x"))
}
fn with_address(signature: &str, a: Address) -> Vec<u8> {
    let mut d = keccak256(signature)[..4].to_vec();
    d.extend_from_slice(a.into_word().as_slice());
    d
}
/// The registry's record of `me` (its `recordWords` view at the head).
pub fn record(rpc: &Rpc, me: Address) -> Result<Option<Record>, String> {
    Record::decode(&call(rpc, me, apply::program_address(0), with_address("recordWords(address)", me))?)
}
/// The registry's removalStatus(victim) view at the head (0 removable), as the wallets' quote asks it.
pub fn removal_status(rpc: &Rpc, me: Address, victim: Address) -> Result<u64, String> {
    let out = call(rpc, me, apply::program_address(0), with_address("removalStatus(address)", victim))?;
    if out.len() != 32 {
        return Err("The registry could not judge the member".into());
    }
    u64::try_from(U256::from_be_slice(&out)).map_err(|_| "The registry could not judge the member".into())
}
/// The facts [`decide`] needs, from the node at `rpc`. The head is read first, so a later answer can only be newer.
pub fn gather(rpc: &Rpc, me: Address, reward: Address) -> Result<Facts, String> {
    let params = rpc.call("pocol_getParams", json!([]))?;
    let head = rpc.call("eth_getBlockByNumber", json!(["latest", false]))?;
    let me_hex = format!("{me:#x}");
    let removable = rpc.call("pocol_getRemovable", json!([]))?;
    // PAY-02 (pay.rs `durable`): nonce > 0 or code, on the state the registration runs on (the head's, or a later one).
    let reward_durable = reward == me
        || small(&rpc.call("eth_getTransactionCount", json!([format!("{reward:#x}"), "latest"]))?, "eth_getTransactionCount")? > 0
        || !rpc.code(reward)?.is_empty();
    Ok(Facts {
        chain_id: params["chainId"].as_u64().ok_or("pocol_getParams: no chainId")?,
        genesis_hash: params["genesisHash"].as_str().ok_or("pocol_getParams: no genesisHash")?.to_owned(),
        peers: small(&rpc.call("net_peerCount", json!([]))?, "net_peerCount")?,
        head_height: small(&head["number"], "head number")?,
        head_ts: small(&head["timestamp"], "head timestamp")?,
        now: crate::node::now_s(),
        record: record(rpc, me)?,
        balance: quantity(&rpc.call("eth_getBalance", json!([me_hex, "latest"]))?, "eth_getBalance")?,
        nonce_latest: small(&rpc.call("eth_getTransactionCount", json!([me_hex, "latest"]))?, "eth_getTransactionCount")?,
        nonce_pending: small(&rpc.call("eth_getTransactionCount", json!([me_hex, "pending"]))?, "eth_getTransactionCount")?,
        base_next: rpc.base_next()?,
        seats: Seats {
            active: removable["active"].as_u64().ok_or("pocol_getRemovable: no active count")?,
            seats: removable["seats"].as_u64().ok_or("pocol_getRemovable: no seat count")?,
            full: removable["full"].as_bool().ok_or("pocol_getRemovable: no full flag")?,
            removable: addresses(&removable["removable"], "pocol_getRemovable removable")?,
            // A node without pendingVictims (an older release) names none; the registry still refuses a taken victim.
            pending_victims: match removable.get("pendingVictims") { Some(v) => addresses(v, "pocol_getRemovable pendingVictims")?, None => vec![] },
        },
        reward_durable,
    })
}

/// `status`: the identity, its record and whether it mines, the account and the seats; it reads only.
pub fn status_json(net: &Network, facts: &Facts, me: Address) -> Value {
    let (state, next) = match &facts.record {
        None => ("unregistered", "Not registered. Review the registration with: pocol register quote"),
        Some(r) if r.eligible() => ("eligible", "Registered and eligible: the running node mines with this identity."),
        Some(_) => ("registered-not-eligible", "Registered but not eligible to mine (exited, inactive or punished). The wallets' Rejoin mining or a new identity applies; this command never registers again."),
    };
    json!({"type":"status","address":format!("{me:#x}"),"state":state,"next":next,"registered":facts.record.is_some(),
        "miningEligible":facts.record.as_ref().is_some_and(Record::eligible),"record":facts.record.as_ref().map(Record::json),
        "balance":coins(facts.balance),"nonce":facts.nonce_latest,"pendingTransactions":facts.nonce_pending.saturating_sub(facts.nonce_latest),
        "network":{"chainId":facts.chain_id,"genesisHash":facts.genesis_hash,"matchesHome":facts.chain_id==net.chain_id&&facts.genesis_hash.eq_ignore_ascii_case(&net.genesis_hash)},
        "head":{"height":facts.head_height,"ageSeconds":facts.now.saturating_sub(facts.head_ts)},"peers":facts.peers,
        "seats":{"active":facts.seats.active,"seats":facts.seats.seats,"full":facts.seats.full,"removable":facts.seats.removable.len(),"pendingReplacements":facts.seats.pending_victims.len()},
        "bondTarget":coins(net.b_reg)})
}

/// A uniform index below n from the system's secure randomness (the replaced member), as the wallets draw it.
fn random_index(n: usize) -> Result<usize, String> {
    let mut b = [0u8; 8];
    getrandom::getrandom(&mut b).map_err(|_| "Secure randomness unavailable")?;
    Ok((u64::from_le_bytes(b) % (n.max(1) as u64)) as usize)
}

/// Whether a typed answer `answer`, given `elapsed` after the quote was shown, signs it: exactly `yes` (its line end
/// aside) within the quote lifetime. Anything else, end of input or Ctrl-C (None) cancels, and nothing is signed.
pub fn confirmed(answer: Option<&str>, elapsed: Duration) -> Result<(), String> {
    match answer {
        Some(line) if line.strip_suffix('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)).unwrap_or(line) == "yes" => {}
        _ => return Err("Cancelled; nothing was signed or sent.".into()),
    }
    if elapsed > QUOTE_LIFETIME {
        return Err("The quote expired before it was confirmed; nothing was signed. Run pocol register confirm again.".into());
    }
    Ok(())
}
/// The receipt of `hash`, polled until RECEIPT_WAIT passes or `stop` (Ctrl-C) is set.
fn wait_receipt(rpc: &Rpc, hash: revm::primitives::B256, stop: &dyn Fn() -> bool) -> Result<Value, String> {
    let start = Instant::now();
    loop {
        let r = rpc.call("eth_getTransactionReceipt", json!([format!("{hash}")]))?;
        if !r.is_null() {
            return Ok(r);
        }
        if start.elapsed() > RECEIPT_WAIT || stop() {
            return Err("not included yet".into());
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}
/// The options of one `pocol register` run.
pub struct Options<'a> {
    pub rpc: SocketAddr,
    pub bond: U256,
    pub reward: Option<Address>,
    /// `confirm`: the typed answer (one line from stdin, read after the quote is printed); None when cancelled.
    pub answer: &'a dyn Fn() -> Result<Option<String>, String>,
    pub out: &'a dyn Fn(&Value),
    /// Ctrl-C: stops the wait for the receipt (the transaction is already sent).
    pub stop: &'a dyn Fn() -> bool,
}

/// One `pocol register SUB` run on `home` (`status`, `quote` or `confirm`).
pub fn run(home: &HomeReader, sub: &str, o: &Options) -> Result<(), String> {
    let net = Network::from_spec(&home.invitation()?.genesis_spec()?)?;
    let me = home.address()?;
    let reward = o.reward.unwrap_or(me);
    let rpc = Rpc { addr: o.rpc.to_string() };
    let quote = || -> Result<Plan, String> {
        let facts = gather(&rpc, me, reward)?;
        let mut drawn = Ok(0);
        let plan = decide(&net, &facts, me, reward, o.bond, |n| { drawn = random_index(n); *drawn.as_ref().unwrap_or(&0) }, |victim| removal_status(&rpc, me, victim));
        drawn?;
        plan
    };
    match sub {
        "status" => {
            (o.out)(&status_json(&net, &gather(&rpc, me, reward)?, me));
            Ok(())
        }
        "quote" => {
            let mut q = quote()?.json();
            q["confirm"] = json!("Nothing was signed. To sign and send this registration run pocol register confirm with the same options, and type yes.");
            (o.out)(&q);
            Ok(())
        }
        "confirm" => {
            // The key is read (and checked against the address marker) before the quote, so a missing key or
            // credential is refused before anything is asked.
            let key = home.key()?;
            let plan = quote()?;
            let quoted = Instant::now();
            let mut q = plan.json();
            q["confirm"] = json!("Type yes and press Enter to sign and send this registration; anything else cancels.");
            (o.out)(&q);
            confirmed((o.answer)()?.as_deref(), quoted.elapsed())?;
            let raw = plan.tx.sign_1559(&key);
            let hash = rpc.send(&raw)?;
            (o.out)(&json!({"type":"sent","hash":format!("{hash}"),"note":"Sent to the node; waiting for inclusion."}));
            match wait_receipt(&rpc, hash, o.stop) {
                Ok(receipt) => {
                    let ok = receipt["status"].as_str() == Some("0x1");
                    let block = receipt["blockNumber"].as_str().and_then(|b| u64::from_str_radix(b.trim_start_matches("0x"), 16).ok());
                    let registered = record(&rpc, me).ok().flatten();
                    (o.out)(&json!({"type":"receipt","hash":format!("{hash}"),"status":if ok{"included"}else{"reverted"},"block":block,
                        "registered":registered.is_some(),"miningEligible":registered.as_ref().is_some_and(Record::eligible),
                        "note":if ok{"Registered. The running node mines with this identity from its next assignment; pocol register status shows the record."}
                            else{"The registry reverted the registration (the per-block limit, a full set, or a replaced member that mined or was taken first). Only the fee was spent; review pocol register quote again."}}));
                    if ok { Ok(()) } else { Err("the registration reverted".into()) }
                }
                Err(_) => Err(format!("transaction {hash} was sent but is not included yet; check pocol register status later (this command never sends it again)")),
            }
        }
        other => Err(format!("unknown pocol register action {other}: expected status, quote or confirm")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn net() -> Network {
        Network { chain_id: 7, genesis_hash: "0xabc".into(), b_reg: U256::from(20u64) * U256::from(10u64).pow(U256::from(18u64)), m_max: 3, theta_inact: 360, u: 380 }
    }
    fn me() -> Address { Address::repeat_byte(0x11) }
    fn facts() -> Facts {
        Facts { chain_id: 7, genesis_hash: "0xABC".into(), peers: 2, head_height: 100, head_ts: 1_000, now: 1_010, record: None,
            balance: U256::from(2_000_000_000_000_000u64), nonce_latest: 0, nonce_pending: 0, base_next: U256::from(GWEI),
            seats: Seats { active: 1, seats: 3, full: false, removable: vec![], pending_victims: vec![] }, reward_durable: true }
    }
    fn plan(f: &Facts, bond: U256) -> Result<Plan, String> {
        decide(&net(), f, me(), me(), bond, |_| 0, |_| Ok(0))
    }
    #[test]
    fn below_a_full_set_it_signs_register_with_the_wallets_gas_and_fee() {
        let p = plan(&facts(), U256::ZERO).unwrap();
        assert_eq!(p.tx.gas, EARNIN_REGISTER_GAS);
        assert_eq!(p.tx.data, register_calldata(me()));
        assert_eq!(&p.tx.data[..4], &keccak256("register(address)")[..4]);
        assert_eq!(p.tx.to, Some(apply::program_address(0)));
        assert_eq!(p.tx.max_priority, U256::from(GWEI));
        assert_eq!(p.tx.max_fee, U256::from(3 * GWEI));
        assert_eq!((p.tx.chain_id, p.tx.nonce, p.tx.value), (7, 0, U256::ZERO));
        // 400,000 gas at 3 gwei is 0.0012 PCOL: the faucet's 0.002 PCOL covers it while the base fee is below 2 gwei.
        assert_eq!(p.json()["maximumFee"], "0.0012");
        assert_eq!(p.json()["method"], "register");
        assert_eq!(p.json()["probation"], true);
        // At exactly 2 gwei the maximum fee is the whole 0.002 PCOL; one wei more is refused.
        let mut f = facts();
        f.base_next = U256::from(2 * GWEI);
        assert_eq!(plan(&f, U256::ZERO).unwrap().json()["maximumFee"], "0.002");
        f.base_next = U256::from(2 * GWEI + 1);
        assert!(plan(&f, U256::ZERO).unwrap_err().contains("Insufficient available balance"));
    }
    #[test]
    fn every_refusal_comes_before_anything_is_signed() {
        let full = net().b_reg;
        let cases: Vec<(Box<dyn Fn(&mut Facts)>, U256, &str)> = vec![
            (Box::new(|f| f.chain_id = 8), U256::ZERO, "another network"),
            (Box::new(|f| f.genesis_hash = "0xdef".into()), U256::ZERO, "another network"),
            (Box::new(|f| f.record = Some(Record { status: 1, tomb: false, bond_state: 4, last_share: 0, registered_at: 1, reward: me(), bond: U256::ZERO, exit_height: 0, due_height: 0 })), U256::ZERO, "already has a registry record"),
            (Box::new(|_| {}), full + U256::from(1u8), "The bond is at most 20 PCOL"),
            (Box::new(|f| f.peers = 0), U256::ZERO, "no peers"),
            (Box::new(|f| f.now = f.head_ts + HEAD_MAX_AGE_S + 1), U256::ZERO, "chain is behind"),
            (Box::new(|f| f.nonce_pending = 1), U256::ZERO, "waiting in the node's pool"),
            (Box::new(|f| f.balance = U256::ZERO), U256::ZERO, "Insufficient available balance"),
            (Box::new(|_| {}), full, "Insufficient available balance"),
            (Box::new(|f| f.seats = Seats { active: 3, seats: 3, full: true, removable: vec![], pending_victims: vec![] }), U256::ZERO, "none can be replaced yet"),
            (Box::new(|f| f.seats = Seats { active: 3, seats: 3, full: true, removable: vec![Address::repeat_byte(9)], pending_victims: vec![Address::repeat_byte(9)] }), U256::ZERO, "already named by a pending replacement"),
        ];
        for (i, (change, bond, expect)) in cases.into_iter().enumerate() {
            let mut f = facts();
            change(&mut f);
            let e = plan(&f, bond).unwrap_err();
            assert!(e.contains(expect), "case {i}: {e}");
        }
        // The head exactly at the age limit is still fresh.
        let mut f = facts();
        f.now = f.head_ts + HEAD_MAX_AGE_S;
        assert!(plan(&f, U256::ZERO).is_ok());
        // A reward account the registry would refuse (RewardNotDurable: no transaction and no code) is refused first;
        // the identity itself never is (R9: a self reward signed by the identity is durable).
        let mut f = facts();
        f.reward_durable = false;
        assert!(decide(&net(), &f, me(), Address::repeat_byte(0x22), U256::ZERO, |_| 0, |_| Ok(0)).unwrap_err().contains("cannot receive registry rewards yet"));
        assert!(decide(&net(), &f, me(), me(), U256::ZERO, |_| 0, |_| Ok(0)).is_ok());
    }
    #[test]
    fn at_a_full_set_it_replaces_a_removable_member_not_already_pending() {
        let (a, b, c) = (Address::repeat_byte(0xa), Address::repeat_byte(0xb), Address::repeat_byte(0xc));
        let mut f = facts();
        f.seats = Seats { active: 3, seats: 3, full: true, removable: vec![a, b, c], pending_victims: vec![b] };
        let mut drawn = 0;
        let judged = std::cell::Cell::new(Address::ZERO);
        let p = decide(&net(), &f, me(), me(), U256::ZERO, |n| { drawn = n; 1 }, |v| { judged.set(v); Ok(0) }).unwrap();
        assert_eq!(drawn, 2, "the pending victim is left out of the draw");
        assert_eq!(judged.get(), c, "index 1 of [a, c]");
        assert_eq!(p.tx.gas, REPLACE_GAS);
        assert_eq!(p.tx.data, replacing_calldata(me(), c));
        assert_eq!(p.tx.data.len(), 68);
        let q = p.json();
        assert_eq!(q["method"], "registerReplacing");
        assert_eq!(q["replacing"]["victim"], format!("{c:#x}"));
        assert_eq!(q["replacing"]["removable"], 2);
        // The registry's own view refuses: nothing is signed.
        assert!(decide(&net(), &f, me(), me(), U256::ZERO, |_| 0, |_| Ok(4)).unwrap_err().contains("code 4"));
        assert!(decide(&net(), &f, me(), me(), U256::ZERO, |_| 0, |_| Err("unreadable".into())).unwrap_err().contains("unreadable"));
        // A non-self reward is the reward word of the call.
        let r = Address::repeat_byte(0x33);
        let p = decide(&net(), &f, me(), r, U256::ZERO, |_| 0, |_| Ok(0)).unwrap();
        assert_eq!(p.tx.data, replacing_calldata(r, a));
    }
    #[test]
    fn only_a_typed_yes_within_the_quote_lifetime_signs() {
        let now = Duration::from_secs(1);
        for yes in ["yes", "yes\n", "yes\r\n"] {
            assert!(confirmed(Some(yes), now).is_ok(), "{yes:?}");
        }
        for no in [None, Some(""), Some("\n"), Some("no\n"), Some("YES\n"), Some("y\n"), Some(" yes\n"), Some("yes \n"), Some("yes\n\n"), Some("yes please\n")] {
            assert!(confirmed(no, now).unwrap_err().contains("nothing was signed"), "{no:?}");
        }
        assert!(confirmed(Some("yes\n"), QUOTE_LIFETIME).is_ok());
        assert!(confirmed(Some("yes\n"), QUOTE_LIFETIME + Duration::from_millis(1)).unwrap_err().contains("expired"));
    }
    #[test]
    fn record_words_decode_as_the_registry_lays_them_out() {
        assert_eq!(Record::decode(&[0u8; 128]).unwrap(), None);
        assert!(Record::decode(&[0u8; 96]).is_err());
        // status 1, live, ACCRUING, lastShare 77 (bits 88..151), registeredAt 66 (bits 152..215).
        let w0 = U256::from(1u8) | (U256::from(4u8) << 16) | (U256::from(77u64) << 88) | (U256::from(66u64) << 152);
        let w3 = U256::from(5u64) | (U256::from(6u64) << 64);
        let mut out = Vec::new();
        for w in [w0, U256::from_be_slice(me().into_word().as_slice()), U256::from(9u64), w3] {
            out.extend_from_slice(&w.to_be_bytes::<32>());
        }
        let r = Record::decode(&out).unwrap().unwrap();
        assert_eq!(r, Record { status: 1, tomb: false, bond_state: 4, last_share: 77, registered_at: 66, reward: me(), bond: U256::from(9u64), exit_height: 5, due_height: 6 });
        assert!(r.eligible());
        for (status, tomb, state) in [(2u8, false, 4u8), (1, true, 4), (1, false, 2)] {
            assert!(!Record { status, tomb, bond_state: state, ..r.clone() }.eligible());
        }
        assert!(Record { bond_state: 1, ..r }.eligible());
    }
    #[test]
    fn options_are_validated() {
        for bad in ["", "0x", "0x00", "1111111111111111111111111111111111111111", "0x000000000000000000000000000000000000zz11",
            "0x0000000000000000000000000000000000000000", "0x000000000000000000000000000000000000ffff", "0x0000000000000000000000000000000000c0c001", "0x0000000000000000000000000000000000c0c0ff"] {
            assert!(parse_reward(bad).is_err(), "{bad}");
        }
        assert_eq!(parse_reward("0x0000000000000000000000000000000000010000").unwrap(), Address::from_word(U256::from(0x10000u64).into()));
        assert_eq!(parse_bond(None).unwrap(), U256::ZERO);
        assert_eq!(parse_bond(Some("0")).unwrap(), U256::ZERO);
        assert_eq!(parse_bond(Some("1.5")).unwrap(), U256::from(1_500_000_000_000_000_000u64));
        for bad in ["-1", "1e3", "0x10", "1.0000000000000000001", "abc"] {
            assert!(parse_bond(Some(bad)).is_err(), "{bad}");
        }
        assert_eq!(parse_rpc(None).unwrap().to_string(), DEFAULT_RPC);
        assert!(parse_rpc(Some("[::1]:8545")).is_ok());
        for bad in ["0.0.0.0:8545", "192.0.2.1:8545", "127.0.0.1:0", "localhost:8545", "127.0.0.1"] {
            assert!(parse_rpc(Some(bad)).is_err(), "{bad}");
        }
    }
    #[test]
    fn status_reports_the_record_and_the_seats_without_signing() {
        let mut f = facts();
        let s = status_json(&net(), &f, me());
        assert_eq!((s["state"].as_str(), s["registered"].as_bool(), s["network"]["matchesHome"].as_bool()), (Some("unregistered"), Some(false), Some(true)));
        f.record = Some(Record { status: 1, tomb: false, bond_state: 1, last_share: 0, registered_at: 1, reward: me(), bond: net().b_reg, exit_height: 0, due_height: 0 });
        f.chain_id = 8;
        let s = status_json(&net(), &f, me());
        assert_eq!((s["state"].as_str(), s["miningEligible"].as_bool(), s["record"]["bond"].as_str(), s["network"]["matchesHome"].as_bool()), (Some("eligible"), Some(true), Some("20"), Some(false)));
    }
}

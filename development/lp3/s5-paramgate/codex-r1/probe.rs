//! Review adapter only; no independent expected arithmetic here.
use lp1_node::param_gate::RpcLimits;
use serde_json::{json, Value};
use std::io::{self, BufRead};
fn main() {
    for line in io::stdin().lock().lines() {
        let v: Value = serde_json::from_str(&line.unwrap()).unwrap();
        let mut r = RpcLimits::default();
        macro_rules! fields { ($($f:ident),*) => { $(if let Some(n) = v.get(stringify!($f)) { r.$f = n.as_u64().unwrap(); })* }; }
        fields!(conn_max,req_buf,conn_buf,send_sock_buf,heavy_active,light_active,light_queue,
          heavy_resp_mem,light_resp_mem,resp_max,light_resp_max,batch_resp_max,scratch_heavy,
          scratch_light,mem_total,batch_max,err_item_max,logs_count_max,logs_scan_bytes,rate,
          bucket_cap,queue_wait_ms,compute_heavy_ms,cancel_max_ms,grace_ms,min_send_rate,
          batch_wall_ms,enforce_slack_ms,timer_eps_ms);
        let out = match r.derive() {
            Err(e) => json!({"error":format!("{:?}",e),"r26":r.satisfies_r26()}),
            Ok(m) => {
                let ratio = |t: lp1_node::param_gate::Milliseconds| {
                    let (n,d) = t.ratio(); json!([n.to_string(),d.to_string()])
                };
                json!({"memory":m.memory_required.to_string(),"hard":m.batch_bytes_hard.to_string(),
                    "send":ratio(m.batch_send_max),"watch":ratio(m.batch_send_watchdog_max),
                    "wall":ratio(m.batch_wall_min),"r26":r.satisfies_r26()})
            }
        };
        println!("{}",out);
    }
}

extern crate lp1_node;
use lp1_node::timing::{self, AttemptWindow};
use std::io::{self, BufRead};
fn option<T: std::fmt::Display>(v: Option<T>) -> String { v.map(|n| n.to_string()).unwrap_or_else(|| "-".into()) }
fn main() {
    for line in io::stdin().lock().lines() {
        let line=line.unwrap();let v:Vec<u128>=line.split_whitespace().map(|s|s.parse().unwrap()).collect();
        let (p,a,d,c,g,f,r,rel,delta,phi,exec,arrival)=(v[0] as u64,v[1] as u32,v[2] as u32,v[3] as u64,v[4] as u64,v[5] as u32,v[6],v[7] as u64,v[8] as u64,v[9] as u64,v[10] as u64,v[11]);
        let mut out=vec!["-".to_string();7];let mut tj=None;
        if let Ok(w)=AttemptWindow::new(p,a,d) {
            out[0]=w.lower().to_string();out[1]=w.upper().to_string();out[2]=option(w.fallback_timestamp(f).ok());
            out[3]=option(w.proposer_timestamp(c).ok());tj=w.deadline_ms(r,rel,f).ok();out[4]=option(tj);
            out[5]=(w.advance_due(c,g) as u8).to_string();out[6]=(w.later_fallback_due(c) as u8).to_string();
        }
        out.push(match timing::max_attempt(p,d) {Err(_)=>"E".into(),Ok(None)=>"N".into(),Ok(Some(n))=>n.to_string()});
        out.push((timing::is_future(c,p,phi) as u8).to_string());
        let m=timing::metrics(delta,phi,exec,rel,f as u64);
        out.extend([m.sigma.to_string(),m.x_h.to_string(),m.x_w.to_string()]);
        out.push(option(tj.map(|t| timing::arrives_by_deadline(arrival,t) as u8)));
        println!("{}",out.join(" "));
    }
}

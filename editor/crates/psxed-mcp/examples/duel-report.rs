//! Re-read a saved guest log without rebuilding or replaying the encounter.
fn main() {
    let path=std::env::args().nth(1).expect("usage: duel-report <guest.log>");
    let log=std::fs::read_to_string(path).expect("read guest log");
    println!("{}",serde_json::to_string_pretty(&psxed_mcp::duel::summarize(&log)).unwrap());
}

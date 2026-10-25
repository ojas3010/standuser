//! Prints the 17 ability templates (name, canon Stand, canon quote, sample card sentence) as JSON for tools/naming/lexicons.py.
use standuser_core::ability::{all_sample_sentences, template};

fn main() {
    let keys = [("PWR", "hi"), ("PWR", "lo"), ("SPD", "hi"), ("SPD", "inf"), ("SPD", "lo"), ("RNG", "hi"), ("RNG", "inf"), ("RNG", "lo"), ("STA", "hi"),
        ("STA", "inf"), ("STA", "lo"), ("PRC", "hi"), ("PRC", "lo"), ("DEV", "hi"), ("DEV", "lo"), ("DEV", "complete"), ("BAL", "bal")];
    let sentences = all_sample_sentences();
    let out: Vec<serde_json::Value> = keys
        .iter()
        .enumerate()
        .map(|(i, (s, d))| {
            let t = template(s, d);
            let sent = sentences[i].strip_prefix(&format!("{}. ", t.name)).and_then(|x| x.strip_suffix(" [provisional]")).unwrap_or("");
            serde_json::json!({"id": t.id, "name": t.name, "canon": t.canon, "quote": t.quote, "sentence": sent})
        })
        .collect();
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
}

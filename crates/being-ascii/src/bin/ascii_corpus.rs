//! Foreground: build a TEACHER-distillation corpus (spends `claude -p` draw calls). Run:
//!   cargo run -p being-ascii --bin ascii_corpus --release
//!
//! Reality check (2026-06-22): Claude draws good ASCII; qwen does not. Self-distillation is capped at
//! qwen's ceiling, so the ceiling-breaking lever is TEACHER distillation — LoRA-tune qwen on Claude's
//! drawings. This bin has Claude draw each subject, structural-gates the result, and writes
//! `{prompt, completion}` JSONL (the format scripts/distill_lora.sh / mlx_lm.lora expects).
use being_ascii::{corpus_line, extract_art, AsciiArt, StructuralGate};
use std::fs;
use std::process::Command;

fn claude_draw(subject: &str) -> String {
    let prompt = format!(
        "Draw a {subject} as ASCII art, 6-12 lines, recognizable. \
         Output ONLY the art (no commentary, no code fences)."
    );
    match Command::new("claude").arg("-p").arg(&prompt).output() {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
        _ => String::new(),
    }
}

fn main() {
    let subjects = [
        "cat", "dog", "house", "tree", "fish", "star", "heart", "flower", "sun", "boat", "car",
        "bird", "face", "mountain", "cup", "key", "umbrella", "robot", "snake", "apple",
    ];
    let gate = StructuralGate::default();
    let dir = ".yogi/ascii_corpus";
    fs::create_dir_all(dir).unwrap();
    eprintln!(
        "teacher corpus: Claude draws {} subjects (claude -p)...",
        subjects.len()
    );

    let mut rows: Vec<String> = Vec::new();
    for subj in subjects {
        let art = AsciiArt::parse(&extract_art(&claude_draw(subj)));
        let ok = gate.check(&art).is_ok();
        println!(
            "  {subj}: {} lines — {}",
            art.height(),
            if ok { "kept" } else { "rejected" }
        );
        if ok {
            rows.push(corpus_line(
                &format!("Draw a {subj} as ASCII art. Output only the art."),
                &format!("\n{}", art.render()),
            ));
        }
    }

    // 80/10/10 split (mlx_lm.lora wants train + valid; test for the ASCII eval later).
    let n = rows.len();
    let tr = n * 8 / 10;
    let va = tr + (n - tr).div_ceil(2);
    let write = |name: &str, rs: &[String]| {
        let body = if rs.is_empty() {
            String::new()
        } else {
            rs.join("\n") + "\n"
        };
        fs::write(format!("{dir}/{name}.jsonl"), body).unwrap();
    };
    write("train", &rows[..tr]);
    write("valid", &rows[tr..va]);
    write("test", &rows[va..]);
    println!("\nteacher corpus: {n} validated drawings → {dir}/{{train,valid,test}}.jsonl");
}

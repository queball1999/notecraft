//! Seeded input generation shared by the fuzz targets' property-test twins. Included with
//! `#[path]` from the crates' `tests/fuzz_props.rs`; not compiled by the fuzz crate.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// Longest input a mutation produces.
pub const MAX_LEN: usize = 256 * 1024;

/// A tiny xorshift64* generator: deterministic from its seed, no dependencies.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A number in `0..n` (0 when `n` is 0).
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }
}

/// Cases per property test: `default`, or the `NOTECRAFT_FUZZ_CASES` environment variable for
/// a longer soak run (`NOTECRAFT_FUZZ_CASES=500000 cargo test --release --test fuzz_props`).
pub fn cases(default: usize) -> usize {
    std::env::var("NOTECRAFT_FUZZ_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// The workspace root, from a crate's manifest dir.
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// Every file under `dir` (recursive, sorted) whose name ends with one of `suffixes`.
pub fn seeds_in(dir: &Path, suffixes: &[&str]) -> Vec<Vec<u8>> {
    let mut files = Vec::new();
    collect(dir, suffixes, &mut files, 0);
    files.sort();
    files.iter().filter_map(|p| std::fs::read(p).ok()).collect()
}

fn collect(dir: &Path, suffixes: &[&str], out: &mut Vec<PathBuf>, depth: usize) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            if depth < 6 {
                collect(&path, suffixes, out, depth + 1);
            }
        } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if suffixes.iter().any(|s| name.ends_with(s)) {
                out.push(path);
            }
        }
    }
}

/// A mutated copy of one of `seeds`: byte flips, inserts, deletes, repeats, truncation,
/// splices from another seed, and fragments from `tokens`.
pub fn mutate(rng: &mut Rng, seeds: &[Vec<u8>], tokens: &[&str]) -> Vec<u8> {
    let mut data = if seeds.is_empty() {
        Vec::new()
    } else {
        seeds[rng.below(seeds.len())].clone()
    };
    for _ in 0..1 + rng.below(6) {
        let at = rng.below(data.len() + 1);
        match rng.below(8) {
            0 if !data.is_empty() => {
                let i = rng.below(data.len());
                data[i] ^= 1 << rng.below(8);
            }
            1 => data.insert(at, (rng.next_u64() & 0xff) as u8),
            2 if !data.is_empty() => {
                let end = (at + 1 + rng.below(64)).min(data.len());
                data.drain(at.min(end)..end);
            }
            3 if !data.is_empty() => {
                let start = rng.below(data.len());
                let end = (start + 1 + rng.below(256)).min(data.len());
                let piece = data[start..end].to_vec();
                for _ in 0..1 + rng.below(40) {
                    if data.len() + piece.len() > MAX_LEN {
                        break;
                    }
                    data.splice(at..at, piece.iter().copied());
                }
            }
            4 => data.truncate(at),
            5 if !seeds.is_empty() => {
                let other = &seeds[rng.below(seeds.len())];
                let start = rng.below(other.len() + 1);
                let end = (start + rng.below(512)).min(other.len());
                data.splice(at..at, other[start..end].iter().copied());
            }
            _ if !tokens.is_empty() => {
                let token = tokens[rng.below(tokens.len())];
                for _ in 0..1 + rng.below(8) {
                    data.splice(at..at, token.bytes());
                }
            }
            _ => {}
        }
        data.truncate(MAX_LEN);
    }
    data
}

/// Random bytes, biased toward printable ASCII and a few multi-byte characters.
pub fn noise(rng: &mut Rng, max: usize) -> Vec<u8> {
    let n = rng.below(max + 1);
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let r = rng.below(100);
        if r < 80 {
            out.push(b' ' + rng.below(95) as u8);
        } else if r < 90 {
            out.push(b"\n\r\t"[rng.below(3)]);
        } else {
            let pieces = ["\u{e9}", "\u{feff}", "\u{1f600}", "\u{0}"];
            out.extend_from_slice(pieces[rng.below(4)].as_bytes());
        }
    }
    out
}

//! Golden and property tests for the canvas block (PR 1.9).
//!
//! - Every `testdata/notecraft/*.notecraft` page reads, and writing back what was read changes
//!   no byte.
//! - `ink-basic.notecraft` plus a fixed new stroke equals `ink-basic.added.golden` (regenerate
//!   with `UPDATE_GOLDEN=1 cargo test -p notecraft-storage --test canvas_roundtrip`).
//! - Random strokes written into random pages read back equal to what the file stores, and
//!   everything outside the `"ink"` value is untouched.
//! - Hostile input (random byte edits, every truncation, deep nesting) never panics, and a
//!   write either is refused or gives a page that reads back.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::path::{Path, PathBuf};

use notecraft_core::{InkColor, InkPoint, InkTool, Stroke};
use notecraft_storage::{
    find_canvas_block, normalized, read_canvas, repair_canvas, strip_canvas_block, write_canvas,
    write_ink, CanvasContainer,
};

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/notecraft")
}

fn pages() -> Vec<(String, String)> {
    let mut out: Vec<_> = std::fs::read_dir(dir())
        .unwrap()
        .flatten()
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("notecraft"))
        .map(|e| {
            (
                e.file_name().to_string_lossy().into_owned(),
                std::fs::read_to_string(e.path()).unwrap(),
            )
        })
        .collect();
    out.sort();
    out
}

/// A small deterministic generator (no extra dependency).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    fn float(&mut self, span: f32) -> f32 {
        (self.next() % 2_000_001) as f32 / 1_000_000.0 * span - span
    }
}

fn random_stroke(rng: &mut Rng, id: usize) -> Stroke {
    let n = 1 + rng.below(40) as usize;
    Stroke {
        texture: Default::default(),
        id: format!("s{id}"),
        tool: if rng.below(4) == 0 {
            InkTool::Highlighter
        } else {
            InkTool::Pen
        },
        color: InkColor([
            rng.below(256) as u8,
            rng.below(256) as u8,
            rng.below(256) as u8,
            if rng.below(3) == 0 { 128 } else { 255 },
        ]),
        width: 0.1 + rng.float(30.0).abs(),
        points: (0..n)
            .map(|_| {
                InkPoint::new(
                    rng.float(2000.0),
                    rng.float(2000.0),
                    rng.float(0.5).abs() * 2.0,
                )
            })
            .collect(),
    }
}

const PAGE_SHAPES: [&str; 5] = [
    "",
    "Plain text\n",
    "---\ntitle: T\n---\n<!-- notecraft:container id=c1 -->\nBody\r\nmore\r\n",
    "text with no newline",
    "Intro\n\n```notecraft-canvas\n{ \"format\": 1, \"containers\": {}, \"z\": [] }\n```\n",
];

#[test]
fn every_sample_page_round_trips_byte_for_byte() {
    let all = pages();
    assert!(all.len() >= 4);
    for (name, text) in all {
        let read = read_canvas(&text);
        let out = write_ink(&text, &read.strokes).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(out, text, "{name}");
        // The same through CRLF line endings.
        let crlf = text.replace('\n', "\r\n");
        let read = read_canvas(&crlf);
        assert_eq!(
            read.strokes.len(),
            read_canvas(&text).strokes.len(),
            "{name}"
        );
        assert_eq!(write_ink(&crlf, &read.strokes).unwrap(), crlf, "{name}");
    }
}

#[test]
fn adding_a_stroke_matches_the_golden_page() {
    let text = std::fs::read_to_string(dir().join("ink-basic.notecraft")).unwrap();
    let mut strokes = read_canvas(&text).strokes;
    strokes.push(Stroke {
        texture: Default::default(),
        id: "s3".to_owned(),
        tool: InkTool::Pen,
        color: InkColor::parse("#C0392B").unwrap(),
        width: 1.3333334,
        points: vec![
            InkPoint::new(12.3456, 40.0, 0.5),
            InkPoint::new(15.0, 41.755, 0.75),
            InkPoint::new(19.5, 44.0, 1.0),
        ],
    });
    let out = write_ink(&text, &strokes).unwrap();
    let golden = dir().join("ink-basic.added.golden");
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&golden, &out).unwrap();
    }
    let expected = std::fs::read_to_string(&golden).expect("golden file");
    assert_eq!(out, expected);
    // Writing the same strokes again changes nothing.
    assert_eq!(write_ink(&out, &read_canvas(&out).strokes).unwrap(), out);
}

#[test]
fn random_strokes_round_trip_and_only_the_ink_value_changes() {
    let mut rng = Rng(0x5eed);
    for round in 0..300 {
        let shape = PAGE_SHAPES[rng.below(PAGE_SHAPES.len() as u64) as usize];
        let count = rng.below(6) as usize;
        let strokes: Vec<Stroke> = (0..count).map(|i| random_stroke(&mut rng, i + 1)).collect();
        let out = write_ink(shape, &strokes).unwrap();
        let read = read_canvas(&out);
        assert_eq!(read.problem, None, "round {round}: {out}");
        let want: Vec<Stroke> = strokes.iter().map(normalized).collect();
        assert_eq!(read.strokes, want, "round {round}");
        // Idempotent.
        assert_eq!(
            write_ink(&out, &read.strokes).unwrap(),
            out,
            "round {round}"
        );
        // Text before the block is untouched; a second, different write keeps it too.
        let before = shape.find("```notecraft-canvas").unwrap_or(shape.len());
        assert_eq!(out.get(..before), shape.get(..before), "round {round}");
        let more: Vec<Stroke> = (0..rng.below(4) as usize)
            .map(|i| random_stroke(&mut rng, i + 100))
            .collect();
        let again = write_ink(&out, &more).unwrap();
        if let (Some(a), Some(b)) = (out.find("\"ink\": "), again.find("\"ink\": ")) {
            assert_eq!(out.get(..a), again.get(..b), "round {round}");
        }
        assert_eq!(
            read_canvas(&again).strokes,
            more.iter().map(normalized).collect::<Vec<_>>()
        );
    }
}

#[test]
fn removing_the_block_leaves_the_markdown() {
    let mut rng = Rng(7);
    for shape in PAGE_SHAPES {
        let strokes = vec![random_stroke(&mut rng, 1)];
        let with = write_ink(shape, &strokes).unwrap();
        let stripped = strip_canvas_block(&with);
        if shape.contains("notecraft-canvas") {
            assert!(!stripped.contains("notecraft-canvas"));
        } else if shape.is_empty() || shape.ends_with('\n') {
            assert_eq!(stripped, shape);
        } else {
            // A page with no final newline gains one when the block goes (the writer had
            // to add a line break before the block).
            assert_eq!(stripped.trim_end_matches(['\r', '\n']), shape);
        }
    }
}

#[test]
fn damaged_blocks_never_panic() {
    let text = std::fs::read_to_string(dir().join("ink-basic.notecraft")).unwrap();
    let probe = vec![random_stroke(&mut Rng(3), 9)];
    // Every truncation, on character boundaries.
    for cut in (0..text.len()).filter(|i| text.is_char_boundary(*i)) {
        let part = &text[..cut];
        let read = read_canvas(part);
        match write_ink(part, &probe) {
            Ok(out) => assert!(read_canvas(&out).problem.is_none(), "cut {cut}"),
            // Refused: a bad block, or a page ending inside an unclosed code fence.
            Err(_) => assert!(
                read.problem.is_some() || find_canvas_block(part).is_none(),
                "cut {cut}"
            ),
        }
    }
    // Random single-byte damage (ASCII replacements keep the text valid UTF-8).
    let mut rng = Rng(99);
    let junk = b"{}[]\",:\\0e-.x\n`";
    for _ in 0..2000 {
        let mut bytes = text.clone().into_bytes();
        for _ in 0..1 + rng.below(4) {
            let at = rng.below(bytes.len() as u64) as usize;
            if bytes.get(at).is_some_and(u8::is_ascii) {
                bytes[at] = junk[rng.below(junk.len() as u64) as usize];
            }
        }
        let Ok(damaged) = String::from_utf8(bytes) else {
            continue;
        };
        let read = read_canvas(&damaged);
        for s in &read.strokes {
            assert!(s.points.len() <= notecraft_core::ink::MAX_STROKE_POINTS);
            assert!(s.points.iter().all(|p| p.x.is_finite() && p.y.is_finite()));
        }
        if let Ok(out) = write_ink(&damaged, &probe) {
            let again = read_canvas(&out);
            assert!(again.problem.is_none(), "{out}");
            assert_eq!(
                again.strokes.len(),
                1,
                "{damaged}
=====
{out}"
            );
        }
    }
}

#[test]
fn block_size_and_depth_limits_hold() {
    let deep = format!(
        "```notecraft-canvas\n{}1{}\n```\n",
        "[".repeat(100_000),
        "]".repeat(100_000)
    );
    assert!(read_canvas(&deep).problem.is_some());
    let long_string = format!(
        "```notecraft-canvas\n{{\"ink\":[{{\"id\":\"{}\",\"points\":[[1,1]]}}]}}\n```\n",
        "x".repeat(1_000_000)
    );
    let read = read_canvas(&long_string);
    assert_eq!(read.strokes.len(), 1);
    assert!(read.strokes[0].id.len() <= 64);
}

fn container(id: &str, x: f32, y: f32, w: f32) -> CanvasContainer {
    CanvasContainer {
        id: id.to_owned(),
        x,
        y,
        w: Some(w),
    }
}

#[test]
fn moving_a_container_matches_the_golden_page() {
    let text = std::fs::read_to_string(dir().join("ink-basic.notecraft")).unwrap();
    let read = read_canvas(&text);
    assert_eq!(read.containers.len(), 2);
    let mut containers = read.containers.clone();
    containers[0] = container("c1", 60.255, 100.0, 400.0);
    containers.push(container("c3", 48.0, 300.0, 300.0));
    let out = write_canvas(&text, &read.strokes, &containers).unwrap();
    let golden = dir().join("containers-moved.golden");
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&golden, &out).unwrap();
    }
    let expected = std::fs::read_to_string(&golden).expect("golden file");
    assert_eq!(out, expected);
    // Only the containers value changed: strokes, shapes, z, and unknown keys are kept.
    assert!(out.contains("\"future\": { \"keep\": [\"me\", 1, null] }"));
    assert!(out.contains("\"shapes\": ["));
    assert_eq!(read_canvas(&out).strokes, read.strokes);
    // Writing the same positions again changes nothing.
    let again = read_canvas(&out);
    assert_eq!(
        write_canvas(&out, &again.strokes, &again.containers).unwrap(),
        out
    );
}

#[test]
fn positions_alone_create_a_block_and_read_back() {
    for shape in PAGE_SHAPES.iter().take(4) {
        let containers = vec![
            container("c2", 520.5, 96.0, 300.0),
            container("c1", 48.0, 96.0, 420.0),
        ];
        let out = write_canvas(shape, &[], &containers).unwrap();
        let read = read_canvas(&out);
        assert_eq!(read.problem, None, "{shape:?}");
        assert_eq!(read.containers.len(), 2);
        assert_eq!(read.containers[0].id, "c1", "written sorted by id");
        assert_eq!(read.containers[1].x, 520.5);
        assert!(read.strokes.is_empty());
        // Nothing to store leaves a page alone.
        assert_eq!(write_canvas(shape, &[], &[]).unwrap(), *shape);
        assert_eq!(strip_canvas_block(&out).trim_end(), shape.trim_end());
    }
}

#[test]
fn clearing_every_position_empties_the_object_and_keeps_the_ink() {
    let text = std::fs::read_to_string(dir().join("ink-basic.notecraft")).unwrap();
    let strokes = read_canvas(&text).strokes;
    let out = write_canvas(&text, &strokes, &[]).unwrap();
    assert!(out.contains("\"containers\": {}"));
    let read = read_canvas(&out);
    assert!(read.containers.is_empty());
    assert_eq!(read.strokes, strokes);
}

#[test]
fn write_ink_keeps_container_positions() {
    let text = std::fs::read_to_string(dir().join("ink-basic.notecraft")).unwrap();
    let out = write_ink(&text, &[]).unwrap();
    assert_eq!(read_canvas(&out).containers, read_canvas(&text).containers);
}

#[test]
fn hostile_container_entries_are_skipped_or_clamped() {
    let text = "```notecraft-canvas\n{\"containers\": {\"a\": {\"x\": 1e30, \"y\": -5}, \"b\": {\"x\": \"no\"}, \"c\": 7, \"d\": {\"x\": 3, \"y\": 4, \"w\": null}, \"a\": {\"x\": 9, \"y\": 9, \"w\": 100}}}\n```\n";
    let read = read_canvas(text);
    assert_eq!(read.problem, None);
    let ids: Vec<&str> = read.containers.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["a", "d"]);
    assert_eq!(read.containers[0].x, 9.0, "the later duplicate wins");
    assert_eq!(read.containers[1].w, None);
    let wide = "```notecraft-canvas\n{\"containers\": {\"a\": {\"x\": 1e30, \"y\": -1e30}}}\n```\n";
    let c = &read_canvas(wide).containers[0];
    assert_eq!((c.x, c.y), (1_000_000.0, -1_000_000.0));
}

#[test]
fn a_broken_block_is_repaired_into_a_fresh_one() {
    let broken = "Intro text\n\n```notecraft-canvas\n{\"ink\": [ {oops\n```\nafter\n";
    assert_eq!(
        read_canvas(broken).problem,
        Some(notecraft_storage::CanvasProblem::Unparseable)
    );
    let fixed = repair_canvas(broken).unwrap();
    let read = read_canvas(&fixed);
    assert_eq!(read.problem, None);
    assert!(fixed.starts_with("Intro text\n\n```notecraft-canvas\n{"));
    assert!(fixed.ends_with("```\nafter\n"));
    // A healthy block, a newer one, and a page with no block are not repaired.
    assert_eq!(repair_canvas(&fixed), None);
    assert_eq!(repair_canvas("plain\n"), None);
    let newer = "```notecraft-canvas\n{\"format\": 99}\n```\n";
    assert_eq!(repair_canvas(newer), None);
    // A cut-short block is repaired too.
    let cut = "text\n```notecraft-canvas\n{\"ink\": [";
    assert!(read_canvas(&repair_canvas(cut).unwrap()).problem.is_none());
}

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;

fn stroke(id: &str, pts: &[(f32, f32, f32)]) -> Stroke {
    Stroke {
        id: id.to_owned(),
        tool: InkTool::Pen,
        color: InkColor::parse("#1F3A93").unwrap(),
        width: 2.0,
        points: pts
            .iter()
            .map(|&(x, y, p)| InkPoint::new(x, y, p))
            .collect(),
    }
}

const BASIC: &str = "Hello\n\n```notecraft-canvas\n{\n  \"format\": 1,\n  \"z\": [\"c1\"],\n  \"ink\": []\n}\n```\n";

#[test]
fn finds_the_last_top_level_canvas_block() {
    let text = "```notecraft-canvas\n{}\n```\ntext\n````md\n```notecraft-canvas\n{}\n```\n````\n```notecraft-canvas\n{\"a\":1}\n```\n";
    let b = find_canvas_block(text).unwrap();
    assert!(b.closed);
    assert_eq!(text.get(b.json).unwrap(), "{\"a\":1}\n");
    assert!(text
        .get(b.block)
        .unwrap()
        .starts_with("```notecraft-canvas"));
    assert!(find_canvas_block("```rust\nfn main() {}\n```\n").is_none());
    assert!(find_canvas_block("").is_none());
}

#[test]
fn an_unclosed_block_runs_to_the_end() {
    let text = "x\n```notecraft-canvas\n{\"ink\": [";
    let b = find_canvas_block(text).unwrap();
    assert!(!b.closed);
    assert_eq!(b.block.end, text.len());
    let read = read_canvas(text);
    assert_eq!(read.problem, Some(CanvasProblem::Unparseable));
    assert_eq!(
        write_ink(text, &[stroke("s1", &[(1.0, 1.0, 1.0)])]),
        Err(WriteInkError::Blocked(CanvasProblem::Unparseable))
    );
}

#[test]
fn reads_the_spec_example() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../testdata/notecraft/ink-basic.notecraft"
    ))
    .unwrap();
    let read = read_canvas(&text);
    assert_eq!(read.problem, None);
    assert_eq!(read.strokes.len(), 2);
    assert_eq!(read.strokes[0].id, "s1");
    assert_eq!(read.strokes[0].points[1], InkPoint::new(604.0, 302.0, 0.47));
    assert_eq!(read.strokes[1].tool, InkTool::Highlighter);
    assert_eq!(read.strokes[1].color.to_hex(), "#FFEB3B80");
}

#[test]
fn unchanged_strokes_leave_the_text_alone() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../testdata/notecraft/ink-basic.notecraft"
    ))
    .unwrap();
    let read = read_canvas(&text);
    assert_eq!(write_ink(&text, &read.strokes).unwrap(), text);
    assert_eq!(write_ink("plain\n", &[]).unwrap(), "plain\n");
}

#[test]
fn adding_a_stroke_changes_only_the_ink_value() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../testdata/notecraft/ink-basic.notecraft"
    ))
    .unwrap();
    let mut strokes = read_canvas(&text).strokes;
    strokes.push(stroke("s3", &[(1.234, 5.0, 0.5), (9.0, 9.999, 0.25)]));
    let out = write_ink(&text, &strokes).unwrap();
    let ink_at = text.find("\"ink\": ").unwrap();
    let after_at = text.find("],\n  \"shapes\"").unwrap() + 1;
    assert_eq!(out.get(..ink_at), text.get(..ink_at));
    let tail = text.get(after_at..).unwrap();
    assert!(out.ends_with(tail));
    let read = read_canvas(&out);
    assert_eq!(read.strokes.len(), 3);
    assert_eq!(read.strokes[2].points[0], InkPoint::new(1.23, 5.0, 0.5));
    assert_eq!(read.strokes[2].points[1], InkPoint::new(9.0, 10.0, 0.25));
    // Unknown keys survive.
    assert!(out.contains("\"future\": { \"keep\": [\"me\", 1, null] }"));
}

#[test]
fn a_block_is_created_only_when_there_is_ink() {
    assert_eq!(write_ink("Hi\n", &[]).unwrap(), "Hi\n");
    let s = stroke("s1", &[(1.0, 2.0, 0.5)]);
    let out = write_ink("Hi", std::slice::from_ref(&s)).unwrap();
    assert!(out.starts_with("Hi\n\n```notecraft-canvas\n{\n  \"format\": 1,\n"));
    assert!(out.ends_with("```\n"));
    assert_eq!(read_canvas(&out).strokes, vec![normalized(&s)]);
    // Empty page text.
    let out = write_ink("", std::slice::from_ref(&s)).unwrap();
    assert!(out.starts_with("```notecraft-canvas\n"));
}

#[test]
fn removing_all_ink_keeps_the_block() {
    let s = stroke("s1", &[(1.0, 2.0, 0.5)]);
    let with = write_ink(BASIC, &[s]).unwrap();
    let without = write_ink(&with, &[]).unwrap();
    assert!(without.contains("\"ink\": []"));
    assert!(read_canvas(&without).strokes.is_empty());
    assert!(without.contains("\"z\": [\"c1\"]"));
}

#[test]
fn a_missing_ink_key_is_inserted() {
    let s = stroke("s1", &[(1.0, 2.0, 0.5)]);
    for text in [
        "```notecraft-canvas\n{ \"format\": 1 }\n```\n",
        "```notecraft-canvas\n{}\n```\n",
        "```notecraft-canvas\n{ \"z\": [] , }\n```\n",
    ] {
        let read = read_canvas(text);
        if read.problem.is_some() {
            continue;
        }
        let out = write_ink(text, std::slice::from_ref(&s)).unwrap();
        assert_eq!(read_canvas(&out).strokes.len(), 1, "{out}");
    }
    let out = write_ink(
        "```notecraft-canvas\n{ \"format\": 1 }\n```\n",
        std::slice::from_ref(&s),
    )
    .unwrap();
    assert!(out.starts_with("```notecraft-canvas\n{ \"format\": 1,\n  \"ink\": ["));
}

#[test]
fn crlf_pages_get_crlf_blocks() {
    let s = stroke("s1", &[(1.0, 2.0, 0.5)]);
    let out = write_ink("Hi\r\nthere\r\n", &[s]).unwrap();
    assert!(!out.replace("\r\n", "").contains('\n'), "{out:?}");
    assert_eq!(read_canvas(&out).strokes.len(), 1);
}

#[test]
fn reading_is_lenient() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../testdata/notecraft/messy-ink.notecraft"
    ))
    .unwrap();
    let read = read_canvas(&text);
    assert_eq!(read.problem, None);
    assert_eq!(read.skipped, 2);
    let ids: Vec<&str> = read.strokes.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["a", "s4", "s5"]);
    let a = &read.strokes[0];
    assert_eq!(a.color, InkColor::default());
    assert_eq!(a.width, 1.0);
    assert_eq!(
        a.points,
        vec![
            InkPoint::new(1.0, 2.0, 1.0),
            InkPoint::new(3.0, 4.0, 1.0),
            InkPoint::new(7.0, 8.0, 0.0)
        ]
    );
    let last = &read.strokes[2];
    assert_eq!(last.tool, InkTool::Highlighter);
    assert_eq!(last.width, MAX_STROKE_WIDTH);
    assert_eq!(last.points[0], InkPoint::new(MAX_COORD, -MAX_COORD, 0.5));
    // Writing what was read is a no-op.
    assert_eq!(write_ink(&text, &read.strokes).unwrap(), text);
}

#[test]
fn long_strokes_are_simplified_and_too_many_strokes_block_saving() {
    let mut pts = String::new();
    for i in 0..30_000 {
        if i > 0 {
            pts.push(',');
        }
        pts.push_str(&format!("[{i},{},0.5]", (i * 37) % 101));
    }
    let text = format!("```notecraft-canvas\n{{\"ink\":[{{\"id\":\"s1\",\"width\":2,\"points\":[{pts}]}}]}}\n```\n");
    let read = read_canvas(&text);
    assert_eq!(read.simplified, 1);
    assert!(read.strokes[0].points.len() <= MAX_STROKE_POINTS);

    let mut many = String::from("```notecraft-canvas\n{\"ink\":[");
    for i in 0..(MAX_STROKES + 5) {
        if i > 0 {
            many.push(',');
        }
        many.push_str(&format!("{{\"id\":\"s{i}\",\"points\":[[1,1]]}}"));
    }
    many.push_str("]}\n```\n");
    let read = read_canvas(&many);
    assert_eq!(read.strokes.len(), MAX_STROKES);
    assert_eq!(
        read.problem,
        Some(CanvasProblem::TooManyStrokes(MAX_STROKES + 5))
    );
    assert!(write_ink(&many, &read.strokes[..1]).is_err());
}

#[test]
fn newer_formats_and_bad_blocks_are_not_rewritten() {
    let newer = "```notecraft-canvas\n{\"format\": 2, \"ink\": []}\n```\n";
    assert_eq!(
        read_canvas(newer).problem,
        Some(CanvasProblem::NewerFormat(2))
    );
    for bad in [
        "```notecraft-canvas\n[1,2]\n```\n",
        "```notecraft-canvas\nnot json\n```\n",
        "```notecraft-canvas\n{\"ink\": [}\n```\n",
        "```notecraft-canvas\n\n```\n",
        "```notecraft-canvas\n{\"a\":1} trailing\n```\n",
    ] {
        let read = read_canvas(bad);
        assert!(read.problem.is_some(), "{bad:?}");
        assert!(write_ink(bad, &[stroke("s1", &[(1.0, 1.0, 1.0)])]).is_err());
    }
}

#[test]
fn deep_and_huge_numbers_do_not_panic() {
    let deep = format!(
        "```notecraft-canvas\n{{\"ink\":{}{}}}\n```\n",
        "[".repeat(5000),
        "]".repeat(5000)
    );
    let r = read_canvas(&deep);
    assert!(
        r.problem.is_some(),
        "{:?} {}",
        r.block.is_some(),
        r.strokes.len()
    );
    let nums = "```notecraft-canvas\n{\"ink\":[{\"width\":1e999,\"points\":[[1e999,2,3],[1e308,-1e308,1e308]]}]}\n```\n";
    let read = read_canvas(nums);
    for s in &read.strokes {
        for p in &s.points {
            assert!(p.x.is_finite() && p.y.is_finite());
        }
    }
}

#[test]
fn stripping_the_block_returns_the_markdown() {
    let s = stroke("s1", &[(1.0, 2.0, 0.5)]);
    let page = "Hi\n\nthere\n";
    let with = write_ink(page, &[s]).unwrap();
    assert_eq!(strip_canvas_block(&with), page);
    assert_eq!(strip_canvas_block(page), page);
    let crlf = "Hi\r\n";
    let with = write_ink(crlf, &[stroke("s1", &[(1.0, 2.0, 0.5)])]).unwrap();
    assert_eq!(strip_canvas_block(&with), crlf);
}

fn object(id: &str, file: bool, src: &str, nth: usize, x: f32, y: f32) -> CanvasObject {
    CanvasObject {
        id: id.to_owned(),
        file,
        src: src.to_owned(),
        nth,
        x,
        y,
    }
}

#[test]
fn placed_pictures_and_files_round_trip_with_minimal_diffs() {
    let page = "Hi\n\n![a](attachments/a.png)\n";
    let objects = vec![
        object("i1", false, "attachments/a.png", 0, 48.0, 96.255),
        object("i2", false, "attachments/a.png", 2, 10.0, 20.0),
        object("f1", true, "attachments/b.pdf", 0, 5.0, 6.0),
    ];
    let out = write_canvas_all(page, &[], &[], &objects).unwrap();
    assert!(out.starts_with(page));
    let read = read_canvas(&out);
    assert_eq!(read.problem, None);
    assert_eq!(read.objects.len(), 3);
    assert_eq!(read.objects[0].id, "i1");
    assert_eq!(read.objects[0].y, 96.26);
    assert_eq!(read.objects[1].nth, 2);
    assert!(read.objects[2].file && read.objects[2].src == "attachments/b.pdf");
    // Written again, nothing changes, byte for byte.
    assert_eq!(
        write_canvas_all(&out, &[], &[], &read.objects).unwrap(),
        out
    );
    // Moving one picture changes only the images list.
    let mut moved = read.objects.clone();
    moved[0].x = 99.0;
    let out2 = write_canvas_all(&out, &[], &[], &moved).unwrap();
    assert_ne!(out2, out);
    let files_line = |t: &str| {
        t.lines()
            .filter(|l| l.contains("b.pdf"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(files_line(&out2), files_line(&out));
    assert_eq!(read_canvas(&out2).objects[0].x, 99.0);
    // Ink and container writers keep the objects they find.
    let inked = write_ink(&out2, &[stroke("s1", &[(1.0, 1.0, 1.0)])]).unwrap();
    assert_eq!(read_canvas(&inked).objects.len(), 3);
    let placed = [CanvasContainer {
        id: "c1".into(),
        x: 1.0,
        y: 2.0,
        w: Some(300.0),
    }];
    let kept = write_canvas(&inked, &read_canvas(&inked).strokes, &placed).unwrap();
    assert_eq!(read_canvas(&kept).objects.len(), 3);
    // Removing every object empties both lists but keeps the block.
    let none = write_canvas_all(&out2, &[], &[], &[]).unwrap();
    assert!(read_canvas(&none).objects.is_empty());
    assert!(find_canvas_block(&none).is_some());
}

#[test]
fn unknown_keys_survive_an_object_change() {
    let text = "x\n\n```notecraft-canvas\n{\n  \"format\": 1,\n  \"shapes\": [ { \"id\": \"sh1\" } ],\n  \"z\": [\"i1\"],\n  \"future\": { \"a\": [1, 2] }\n}\n```\n";
    let out =
        write_canvas_all(text, &[], &[], &[object("i1", false, "a.png", 0, 1.0, 2.0)]).unwrap();
    assert!(out.contains("\"shapes\": [ { \"id\": \"sh1\" } ],"));
    assert!(out.contains("\"future\": { \"a\": [1, 2] }"));
    assert!(out.contains("\"z\": [\"i1\"],"));
    assert_eq!(read_canvas(&out).objects.len(), 1);
}

#[test]
fn reading_objects_is_lenient_and_bounded() {
    let text = "```notecraft-canvas\n{\"format\":1,\"images\":[\
        {\"id\":\"a\",\"src\":\"one.png\",\"x\":1,\"y\":2,\"w\":300,\"h\":9},\
        {\"id\":\"a\",\"src\":\"two.png\",\"x\":3,\"y\":4},\
        {\"src\":\"\",\"x\":1,\"y\":2},\
        {\"src\":\"three.png\",\"x\":\"no\",\"y\":2},\
        {\"src\":\"four.png\",\"y\":2},\
        {\"src\":\"one.png\",\"x\":9,\"y\":9},\
        {\"src\":\"five.png\",\"nth\":-3,\"x\":1e99,\"y\":-1e99},\
        {\"src\":\"six.png\",\"nth\":1.5,\"x\":0,\"y\":0},\
        7, \"x\", null, [1]],\
        \"files\":{\"not\":\"a list\"}}\n```\n";
    let read = read_canvas(text);
    assert_eq!(read.problem, None);
    let srcs: Vec<_> = read.objects.iter().map(|o| o.src.as_str()).collect();
    // Repeats of (src, nth), blanks, and non-numbers are dropped; huge numbers are clamped.
    assert_eq!(srcs, ["one.png", "two.png", "five.png", "six.png"]);
    let ids: HashSet<_> = read.objects.iter().map(|o| o.id.clone()).collect();
    assert_eq!(ids.len(), 4, "ids are unique");
    assert_eq!(read.objects[2].x, MAX_COORD);
    assert_eq!(read.objects[2].y, -MAX_COORD);
    assert_eq!(read.objects[2].nth, 0);
    assert_eq!(read.objects[3].nth, 0);
    // Too many entries: only the first MAX_OBJECTS are kept.
    let many: Vec<String> = (0..MAX_OBJECTS + 50)
        .map(|i| format!("{{\"src\":\"p{i}.png\",\"x\":1,\"y\":1}}"))
        .collect();
    let big = format!(
        "```notecraft-canvas\n{{\"images\":[{}]}}\n```\n",
        many.join(",")
    );
    assert_eq!(read_canvas(&big).objects.len(), MAX_OBJECTS);
    // A very long link is cut.
    let long = format!(
        "```notecraft-canvas\n{{\"images\":[{{\"src\":\"{}\",\"x\":1,\"y\":1}}]}}\n```\n",
        "s".repeat(10_000)
    );
    assert_eq!(
        read_canvas(&long).objects[0].src.chars().count(),
        MAX_OBJECT_SRC
    );
}

#[test]
fn hostile_object_blocks_never_panic_and_are_not_rewritten_when_broken() {
    let broken = "```notecraft-canvas\n{\"images\": [{\"src\": \"a.png\", \"x\": 1, \n```\n";
    let read = read_canvas(broken);
    assert_eq!(read.problem, Some(CanvasProblem::Unparseable));
    assert!(write_canvas_all(
        broken,
        &[],
        &[],
        &[object("i1", false, "a.png", 0, 1.0, 1.0)]
    )
    .is_err());
    let nested = format!(
        "```notecraft-canvas\n{{\"images\":[{}1{}]}}\n```\n",
        "[".repeat(2000),
        "]".repeat(2000)
    );
    assert_eq!(
        read_canvas(&nested).problem,
        Some(CanvasProblem::Unparseable)
    );
}

#[test]
fn objects_in_a_markdown_page_create_the_block() {
    let out = write_canvas_all(
        "Plain\n",
        &[],
        &[],
        &[object("f1", true, "x.pdf", 0, 1.0, 2.0)],
    )
    .unwrap();
    assert!(out.contains("\"files\": ["));
    assert!(!out.contains("\"images\""));
    // No objects and nothing else: no block.
    assert_eq!(
        write_canvas_all("Plain\n", &[], &[], &[]).unwrap(),
        "Plain\n"
    );
}

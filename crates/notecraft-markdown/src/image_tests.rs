#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crate::{block_to_markdown, parse_blocks, Block, BlockKind, LineEnding, Run, Span};

fn runs_of(text: &str) -> Vec<Run> {
    let blocks = parse_blocks(text, 0);
    let Some(BlockKind::Paragraph { runs }) = blocks.first().map(|b| &b.kind) else {
        panic!("expected a paragraph");
    };
    runs.clone()
}

#[test]
fn images_are_runs_with_alt_text_and_a_target() {
    let runs = runs_of(
        "![Board photo](attachments/a81b07-board%20photo.jpg) and ![](x.png) [file](attachments/f.xlsx)",
    );
    let images: Vec<_> = runs.iter().filter(|r| r.image).collect();
    assert_eq!(images.len(), 2);
    assert_eq!(images[0].text, "Board photo");
    assert_eq!(
        images[0].link.as_deref(),
        Some("attachments/a81b07-board%20photo.jpg")
    );
    // An image with no alt text is still an image.
    assert_eq!(images[1].text, "");
    assert_eq!(images[1].link.as_deref(), Some("x.png"));
    // A plain link is not an image.
    assert!(runs
        .iter()
        .any(|r| !r.image && r.text == "file" && r.link.is_some()));
}

#[test]
fn neighboring_images_do_not_merge() {
    let runs = runs_of("![a](x.png)![b](x.png)");
    assert_eq!(runs.iter().filter(|r| r.image).count(), 2);
}

#[test]
fn images_serialize_with_alt_text_and_target() {
    let p = Block {
        span: Span::default(),
        kind: BlockKind::Paragraph {
            runs: vec![Run {
                text: "A [b]".into(),
                link: Some("attachments/a b.png".into()),
                image: true,
                ..Run::default()
            }],
        },
    };
    assert_eq!(
        block_to_markdown(&p, LineEnding::Lf),
        "![A \\[b\\]](attachments/a%20b.png)"
    );
}

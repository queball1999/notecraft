//! `cargo xtask ico <out.ico> <in.png>...` and `cargo xtask icns <out.icns> <in.png>...`:
//! pack rendered PNGs into Windows and macOS icon files (used by packaging/icons.sh).
//!
//! Both formats accept PNG-compressed entries on every supported OS version (Windows Vista and
//! later, macOS 10.7 and later), so each PNG is stored as is. Sizes come from the PNG IHDR
//! chunk. The ICO packer follows the GridCraft xtask (MIT OR Apache-2.0, see ATTRIBUTION.md).

use std::path::Path;

/// Width and height from a PNG's IHDR chunk.
pub fn png_size(png: &[u8]) -> Result<(u32, u32), String> {
    const SIG: &[u8] = b"\x89PNG\r\n\x1a\n";
    let be = |at: usize| -> Option<u32> {
        let b: [u8; 4] = png.get(at..at + 4)?.try_into().ok()?;
        Some(u32::from_be_bytes(b))
    };
    if png.get(..8) != Some(SIG) || png.get(12..16) != Some(b"IHDR".as_slice()) {
        return Err("not a PNG file".into());
    }
    match (be(16), be(20)) {
        (Some(w), Some(h)) => Ok((w, h)),
        _ => Err("truncated PNG header".into()),
    }
}

fn square(png: &[u8]) -> Result<u32, String> {
    let (w, h) = png_size(png)?;
    if w != h || w == 0 {
        return Err(format!("icon images must be square, got {w}x{h}"));
    }
    Ok(w)
}

fn len_u32(n: usize) -> Result<u32, String> {
    u32::try_from(n).map_err(|_| "icon data too large".to_string())
}

/// An ICO file holding `pngs`, which must be square and at most 256 px.
pub fn pack_ico(pngs: &[Vec<u8>]) -> Result<Vec<u8>, String> {
    let count = u16::try_from(pngs.len()).map_err(|_| "too many images".to_string())?;
    if count == 0 {
        return Err("need at least one PNG".into());
    }
    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes()); // reserved
    out.extend_from_slice(&1u16.to_le_bytes()); // type: icon
    out.extend_from_slice(&count.to_le_bytes());
    let mut offset = 6 + 16 * pngs.len();
    for png in pngs {
        let size = square(png)?;
        if size > 256 {
            return Err(format!("ICO images must be at most 256 px, got {size}"));
        }
        // 256 px is written as 0, per the ICO format.
        let dim = u8::try_from(size).unwrap_or(0);
        out.extend_from_slice(&[dim, dim, 0, 0]); // width, height, palette size, reserved
        out.extend_from_slice(&1u16.to_le_bytes()); // color planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        out.extend_from_slice(&len_u32(png.len())?.to_le_bytes());
        out.extend_from_slice(&len_u32(offset)?.to_le_bytes());
        offset += png.len();
    }
    for png in pngs {
        out.extend_from_slice(png);
    }
    Ok(out)
}

/// ICNS element types for PNG payloads, by pixel size. Retina variants share pixel sizes with
/// larger standard ones, so each size maps to every type that uses it.
fn icns_types(size: u32) -> &'static [&'static [u8; 4]] {
    match size {
        16 => &[b"icp4"],
        32 => &[b"icp5", b"ic11"],
        64 => &[b"icp6", b"ic12"],
        128 => &[b"ic07"],
        256 => &[b"ic08", b"ic13"],
        512 => &[b"ic09", b"ic14"],
        1024 => &[b"ic10"],
        _ => &[],
    }
}

/// An ICNS file holding `pngs` (square; 16, 32, 64, 128, 256, 512, or 1024 px).
pub fn pack_icns(pngs: &[Vec<u8>]) -> Result<Vec<u8>, String> {
    if pngs.is_empty() {
        return Err("need at least one PNG".into());
    }
    let mut body = Vec::new();
    for png in pngs {
        let size = square(png)?;
        let types = icns_types(size);
        if types.is_empty() {
            return Err(format!("no ICNS slot for {size} px"));
        }
        for t in types {
            body.extend_from_slice(t.as_slice());
            body.extend_from_slice(&len_u32(png.len() + 8)?.to_be_bytes());
            body.extend_from_slice(png);
        }
    }
    let mut out = Vec::with_capacity(body.len() + 8);
    out.extend_from_slice(b"icns");
    out.extend_from_slice(&len_u32(body.len() + 8)?.to_be_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

fn read_all(inputs: &[&str]) -> Result<Vec<Vec<u8>>, String> {
    inputs
        .iter()
        .map(|p| std::fs::read(p).map_err(|e| format!("read {p}: {e}")))
        .collect()
}

pub fn run(kind: &str, args: &[&str]) -> Result<(), String> {
    let [out, inputs @ ..] = args else {
        return Err(format!(
            "usage: cargo xtask {kind} <out.{kind}> <in.png>..."
        ));
    };
    let pngs = read_all(inputs)?;
    let data = if kind == "icns" {
        pack_icns(&pngs)?
    } else {
        pack_ico(&pngs)?
    };
    std::fs::write(Path::new(out), &data).map_err(|e| format!("write {out}: {e}"))?;
    println!("{out}: {} images, {} bytes", pngs.len(), data.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_png(size: u32) -> Vec<u8> {
        let mut v = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        v.extend_from_slice(&size.to_be_bytes());
        v.extend_from_slice(&size.to_be_bytes());
        v.extend_from_slice(&[8, 6, 0, 0, 0, 1, 2, 3, 4]);
        v
    }

    fn le32(b: &[u8], at: usize) -> u32 {
        u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
    }

    #[test]
    fn ico_directory_and_payloads() {
        let a = fake_png(16);
        let b = fake_png(256);
        let ico = pack_ico(&[a.clone(), b.clone()]).unwrap();
        assert_eq!(&ico[..6], &[0, 0, 1, 0, 2, 0]);
        assert_eq!(&ico[6..8], &[16, 16]);
        assert_eq!(le32(&ico, 14), a.len() as u32);
        assert_eq!(le32(&ico, 18), 38);
        assert_eq!(&ico[22..24], &[0, 0], "256 px is stored as 0");
        assert_eq!(le32(&ico, 34), 38 + a.len() as u32);
        assert_eq!(&ico[38 + a.len()..], &b[..]);
    }

    #[test]
    fn icns_header_and_retina_slots() {
        let p = fake_png(32);
        let icns = pack_icns(std::slice::from_ref(&p)).unwrap();
        assert_eq!(&icns[..4], b"icns");
        assert_eq!(
            u32::from_be_bytes(icns[4..8].try_into().unwrap()) as usize,
            icns.len()
        );
        assert_eq!(&icns[8..12], b"icp5");
        let second = 8 + 8 + p.len();
        assert_eq!(&icns[second..second + 4], b"ic11");
    }

    #[test]
    fn rejects_bad_input() {
        assert!(pack_ico(&[]).is_err());
        assert!(pack_ico(&[fake_png(512)]).is_err());
        assert!(pack_ico(&[b"GIF89a".to_vec()]).is_err());
        assert!(pack_icns(&[fake_png(48)]).is_err());
        assert!(png_size(&fake_png(48)[..20]).is_err());
        assert_eq!(png_size(&fake_png(48)).unwrap(), (48, 48));
    }
}

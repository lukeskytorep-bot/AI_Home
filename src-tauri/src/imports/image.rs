//! Adapted from Harness documents.rs (MIT), see docs/reuse/HARNESS_DOCUMENT_IMPORT_LICENSE.txt.
//! Original bytes are kept; no OCR, resize, recompression or animation truncation.
use std::io::Cursor;
use image::{AnimationDecoder, ImageDecoder, ImageReader, Limits};
pub const MAX_IMAGE_BYTES:usize=10*1024*1024;
const MAX_IMAGE_DIMENSION:u32=16_384;
const MAX_IMAGE_PIXELS:u64=40_000_000;
#[derive(Debug)]
pub(crate) struct ParsedImage{pub mime:&'static str,pub format:&'static str,pub width:u32,pub height:u32}
fn limits()->Limits{let mut l=Limits::default();l.max_image_width=Some(MAX_IMAGE_DIMENSION);l.max_image_height=Some(MAX_IMAGE_DIMENSION);l.max_alloc=Some(192*1024*1024);l}
pub(crate) fn parse(file_name:&str,bytes:&[u8])->Result<ParsedImage,String>{
    if bytes.is_empty()||bytes.len()>MAX_IMAGE_BYTES{return Err("image byte limit exceeded".into())}
    let(mime,width,height)=image_metadata(bytes).ok_or("image damaged or unsupported signature")?;
    if !extension_matches_image(file_name,mime){return Err("image extension does not match signature".into())}
    if width==0||height==0||width>MAX_IMAGE_DIMENSION||height>MAX_IMAGE_DIMENSION||u64::from(width)*u64::from(height)>MAX_IMAGE_PIXELS{return Err("image dimension limit exceeded".into())}
    // Reject animation explicitly. A still-image request must not silently discard frames.
    if mime=="image/png" {let mut i=8usize;while i+12<=bytes.len(){let n=u32::from_be_bytes(bytes[i..i+4].try_into().unwrap()) as usize;if &bytes[i+4..i+8]==b"acTL"{return Err("image animation unsupported".into())}i=i.checked_add(n).and_then(|i|i.checked_add(12)).ok_or("image damaged")?;}}
    if mime=="image/webp" {let mut i=12usize;while i+8<=bytes.len(){let kind=&bytes[i..i+4];if kind==b"ANIM"||kind==b"ANMF"||(kind==b"VP8X"&&bytes.get(i+8).is_some_and(|b|b&2!=0)){return Err("image animation unsupported".into())}let n=u32::from_le_bytes(bytes[i+4..i+8].try_into().unwrap()) as usize;i=i.checked_add(8).and_then(|i|i.checked_add(n)).and_then(|i|i.checked_add(n%2)).ok_or("image damaged")?;}}
    if mime=="image/gif" {
        let mut decoder=image::codecs::gif::GifDecoder::new(Cursor::new(bytes)).map_err(|_|"image damaged")?;
        decoder.set_limits(limits()).map_err(|_|"image decode limit")?;
        let mut frames=decoder.into_frames();
        let frame=frames.next().ok_or("image damaged")?.map_err(|_|"image damaged")?;
        if frame.buffer().width()!=width||frame.buffer().height()!=height{return Err("image decoded dimensions mismatch".into())}
        if let Some(next)=frames.next(){next.map_err(|_|"image damaged")?;return Err("image animation unsupported".into())}
    }else{
        let mut reader=ImageReader::new(Cursor::new(bytes)).with_guessed_format().map_err(|_|"image damaged")?;
        reader.limits(limits());let decoded=reader.decode().map_err(|_|"image data cannot be decoded safely")?;
        if decoded.width()!=width||decoded.height()!=height{return Err("image decoded dimensions mismatch".into())}
    }
    let format=match mime{"image/png"=>"png","image/jpeg"=>"jpeg","image/webp"=>"webp","image/gif"=>"gif",_=>return Err("image unsupported".into())};
    Ok(ParsedImage{mime,format,width,height})
}
pub(crate) fn is_image(file_name:&str)->bool{is_image_extension(file_name)}
fn image_metadata(bytes: &[u8]) -> Option<(&'static str, u32, u32)> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") && bytes.len() >= 24 && &bytes[12..16] == b"IHDR" {
        return Some((
            "image/png",
            u32::from_be_bytes(bytes[16..20].try_into().ok()?),
            u32::from_be_bytes(bytes[20..24].try_into().ok()?),
        ));
    }
    if (bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")) && bytes.len() >= 10 {
        return Some((
            "image/gif",
            u16::from_le_bytes(bytes[6..8].try_into().ok()?) as u32,
            u16::from_le_bytes(bytes[8..10].try_into().ok()?) as u32,
        ));
    }
    if bytes.starts_with(b"\xff\xd8") {
        return jpeg_dimensions(bytes).map(|(width, height)| ("image/jpeg", width, height));
    }
    if bytes.len() >= 30 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return webp_dimensions(bytes).map(|(width, height)| ("image/webp", width, height));
    }
    None
}

fn jpeg_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let mut index = 2usize;
    while index + 9 < bytes.len() {
        if bytes[index] != 0xff {
            index += 1;
            continue;
        }
        let marker = bytes[index + 1];
        index += 2;
        if matches!(marker, 0xd8 | 0xd9) {
            continue;
        }
        if index + 2 > bytes.len() {
            return None;
        }
        let length = u16::from_be_bytes(bytes[index..index + 2].try_into().ok()?) as usize;
        if length < 2 || index + length > bytes.len() {
            return None;
        }
        if matches!(marker, 0xc0 | 0xc1 | 0xc2 | 0xc3 | 0xc5 | 0xc6 | 0xc7 | 0xc9 | 0xca | 0xcb | 0xcd | 0xce | 0xcf) {
            let height = u16::from_be_bytes(bytes[index + 3..index + 5].try_into().ok()?) as u32;
            let width = u16::from_be_bytes(bytes[index + 5..index + 7].try_into().ok()?) as u32;
            return Some((width, height));
        }
        index += length;
    }
    None
}

fn webp_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    match &bytes[12..16] {
        b"VP8X" if bytes.len() >= 30 => {
            let width = 1 + u32::from_le_bytes([bytes[24], bytes[25], bytes[26], 0]);
            let height = 1 + u32::from_le_bytes([bytes[27], bytes[28], bytes[29], 0]);
            Some((width, height))
        }
        b"VP8L" if bytes.len() >= 25 && bytes[20] == 0x2f => {
            let bits = u32::from_le_bytes(bytes[21..25].try_into().ok()?);
            Some(((bits & 0x3fff) + 1, ((bits >> 14) & 0x3fff) + 1))
        }
        b"VP8 " if bytes.len() >= 30 && &bytes[23..26] == b"\x9d\x01\x2a" => Some((
            u16::from_le_bytes(bytes[26..28].try_into().ok()?) as u32 & 0x3fff,
            u16::from_le_bytes(bytes[28..30].try_into().ok()?) as u32 & 0x3fff,
        )),
        _ => None,
    }
}

fn is_image_extension(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    [".png", ".jpg", ".jpeg", ".webp", ".gif"]
        .iter()
        .any(|extension| lower.ends_with(extension))
}

fn extension_matches_image(file_name: &str, mime_type: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    match mime_type {
        "image/png" => lower.ends_with(".png"),
        "image/jpeg" => lower.ends_with(".jpg") || lower.ends_with(".jpeg"),
        "image/webp" => lower.ends_with(".webp"),
        "image/gif" => lower.ends_with(".gif"),
        _ => false,
    }
}


#[cfg(test)]mod tests{
use super::*;
fn encoded(format:image::ImageFormat)->Vec<u8>{let mut b=Cursor::new(vec![]);image::DynamicImage::new_rgb8(3,2).write_to(&mut b,format).unwrap();b.into_inner()}
#[test]fn static_formats_have_verified_signature_dimensions_and_decode(){for(ext,format,mime) in [("png",image::ImageFormat::Png,"image/png"),("jpg",image::ImageFormat::Jpeg,"image/jpeg"),("webp",image::ImageFormat::WebP,"image/webp"),("gif",image::ImageFormat::Gif,"image/gif")]{let bytes=encoded(format);let p=parse(&format!("photo.{ext}"),&bytes).unwrap();assert_eq!((p.mime,p.width,p.height),(mime,3,2));}}
#[test]fn rejects_damage_disguised_files_and_pixel_bombs(){let bytes=encoded(image::ImageFormat::Png);assert!(parse("fake.jpg",&bytes).is_err());assert!(parse("fake.png",b"<svg></svg>").is_err());assert!(parse("damaged.png",&bytes[..24]).is_err());let mut huge=bytes;huge[16..20].copy_from_slice(&20_000u32.to_be_bytes());assert!(parse("huge.png",&huge).unwrap_err().contains("limit"));}
#[test]fn rejects_animated_gif_and_apng_instead_of_silently_using_first_frame(){let mut b=vec![];{let mut encoder=image::codecs::gif::GifEncoder::new(&mut b);for _ in 0..2{encoder.encode_frame(image::Frame::new(image::RgbaImage::new(2,2))).unwrap();}}assert!(parse("animation.gif",&b).unwrap_err().contains("animation"));let mut png=encoded(image::ImageFormat::Png);png.splice(33..33, [0,0,0,0,b'a',b'c',b'T',b'L',0,0,0,0]);assert!(parse("animation.png",&png).unwrap_err().contains("animation"));}
#[test]fn encoded_byte_limit_is_enforced(){assert!(parse("large.png",&vec![0;MAX_IMAGE_BYTES+1]).unwrap_err().contains("limit"));}
}

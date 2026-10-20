//! Adapted from AI RV Harness documents.rs; MIT. No dialogs, providers or RV resources.
use lopdf::Document;
use sha2::{Digest, Sha256};
use std::io::{Cursor, Read};
use zip::ZipArchive;
const MAX_DOCUMENT_BYTES: usize = 15 * 1024 * 1024;
const MAX_TEXT_BYTES: usize = 2 * 1024 * 1024;
const MAX_DOCX_ENTRIES: usize = 512;
const MAX_DOCX_UNCOMPRESSED_BYTES: u64 = 50 * 1024 * 1024;
const MAX_DOCX_XML_BYTES: u64 = 8 * 1024 * 1024;
const MAX_DOCX_COMPRESSION_RATIO: u64 = 120;
const MAX_XML_NODES: usize = 250_000;
const MAX_XML_DEPTH: usize = 128;
const MAX_PDF_PAGES: usize = 200;

#[derive(Debug)]
pub(crate) struct ParsedDocument {
    pub display_name: String,
    pub content: String,
    pub content_hash: String,
    pub mime_type: String,
    pub import_method: String,
    pub size_bytes: usize,
}
fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub(crate) fn parse(file_name: &str, bytes: &[u8]) -> Result<ParsedDocument, String> {
    if bytes.is_empty() || bytes.len() > MAX_DOCUMENT_BYTES {
        return Err("document must be between 1 byte and 15 MB".to_string());
    }
    let lower = file_name.to_lowercase();
    let (_source_type, mime_type, import_method, content) = if bytes.starts_with(b"%PDF-") {
        if !lower.ends_with(".pdf") {
            return Err("file content is PDF but the extension does not match".to_string());
        }
        ("pdf", "application/pdf", "lopdf-text", parse_pdf(bytes)?)
    } else if bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06") {
        if !lower.ends_with(".docx") {
            return Err("file content is a ZIP package but the extension is not .docx".to_string());
        }
        (
            "docx",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "safe-docx-xml",
            parse_docx(bytes)?,
        )
    } else if lower.ends_with(".txt") || lower.ends_with(".md") {
        let text = String::from_utf8(bytes.to_vec())
            .map_err(|_| "text source must use UTF-8 encoding".to_string())?
            .trim_start_matches('\u{feff}')
            .to_string();
        let source_type = if lower.ends_with(".md") {
            "markdown"
        } else {
            "text"
        };
        let mime = if source_type == "markdown" {
            "text/markdown"
        } else {
            "text/plain"
        };
        (source_type, mime, "utf8-text", text)
    } else {
        return Err("unsupported attachment content; choose TXT, MD, text PDF or DOCX".to_string());
    };
    let content = normalize_extracted_text(content)?;
    Ok(ParsedDocument {
        display_name: file_name.to_string(),
        content_hash: sha256(content.as_bytes()),
        content,
        mime_type: mime_type.to_string(),
        import_method: import_method.to_string(),
        size_bytes: bytes.len(),
    })
}

fn parse_docx(bytes: &[u8]) -> Result<String, String> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|_| "DOCX package is damaged or password-protected".to_string())?;
    if archive.is_empty() || archive.len() > MAX_DOCX_ENTRIES {
        return Err("DOCX contains too many package entries".to_string());
    }
    let mut total_uncompressed = 0u64;
    let mut document_index = None;
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|_| "DOCX package entry is damaged".to_string())?;
        let name = entry.name().replace('\\', "/");
        if entry.enclosed_name().is_none() || name.starts_with('/') || name.contains("../") {
            return Err("DOCX contains an unsafe package path".to_string());
        }
        let lower = name.to_ascii_lowercase();
        if lower.contains("vbaproject")
            || lower.starts_with("word/embeddings/")
            || lower.starts_with("word/externallinks/")
            || lower.ends_with(".bin")
        {
            return Err(
                "DOCX contains macros, embedded objects or active external content".to_string(),
            );
        }
        total_uncompressed = total_uncompressed.saturating_add(entry.size());
        if total_uncompressed > MAX_DOCX_UNCOMPRESSED_BYTES {
            return Err("DOCX exceeds the uncompressed-size limit".to_string());
        }
        let compressed = entry.compressed_size().max(1);
        if entry.size() > 1_048_576 && entry.size() / compressed > MAX_DOCX_COMPRESSION_RATIO {
            return Err("DOCX has an unsafe compression ratio".to_string());
        }
        if lower == "word/document.xml" {
            document_index = Some(index);
            if entry.size() > MAX_DOCX_XML_BYTES {
                return Err("DOCX document XML exceeds the safety limit".to_string());
            }
        }
    }
    let index = document_index.ok_or_else(|| "DOCX is missing word/document.xml".to_string())?;
    let entry = archive
        .by_index(index)
        .map_err(|_| "DOCX document XML is unavailable".to_string())?;
    if entry.encrypted() {
        return Err("password-protected DOCX files are not supported".to_string());
    }
    let mut xml = String::new();
    entry
        .take(MAX_DOCX_XML_BYTES + 1)
        .read_to_string(&mut xml)
        .map_err(|_| "DOCX document XML is not valid UTF-8".to_string())?;
    if xml.len() as u64 > MAX_DOCX_XML_BYTES {
        return Err("DOCX document XML exceeds the safety limit".to_string());
    }
    if xml.contains("<!DOCTYPE") || xml.contains("<!ENTITY") {
        return Err("DOCX DTD and entity declarations are not allowed".to_string());
    }
    let document =
        roxmltree::Document::parse(&xml).map_err(|_| "DOCX contains malformed XML".to_string())?;
    let mut node_count = 0usize;
    let mut output = String::new();
    for paragraph in document
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "p")
    {
        let mut paragraph_text = String::new();
        for node in paragraph.descendants() {
            node_count += 1;
            if node_count > MAX_XML_NODES
                || node.ancestors().take(MAX_XML_DEPTH + 1).count() > MAX_XML_DEPTH
            {
                return Err("DOCX XML complexity exceeds the safety limit".to_string());
            }
            if node.is_element() && node.tag_name().name() == "t" {
                if let Some(text) = node.text() {
                    paragraph_text.push_str(text);
                }
            } else if node.is_element() && node.tag_name().name() == "tab" {
                paragraph_text.push('\t');
            } else if node.is_element() && matches!(node.tag_name().name(), "br" | "cr") {
                paragraph_text.push('\n');
            }
        }
        if !paragraph_text.trim().is_empty() {
            output.push_str(paragraph_text.trim_end());
            output.push('\n');
        }
        if output.len() > MAX_TEXT_BYTES {
            return Err("DOCX extracted text exceeds the 2 MB limit".to_string());
        }
    }
    Ok(output)
}

fn parse_pdf(bytes: &[u8]) -> Result<String, String> {
    let document = Document::load_mem(bytes)
        .map_err(|_| "PDF is damaged, encrypted or unsupported".to_string())?;
    if document.is_encrypted() {
        return Err("password-protected or encrypted PDF files are not supported".to_string());
    }
    let pages = document.get_pages();
    if pages.is_empty() {
        return Err("PDF contains no pages".to_string());
    }
    if pages.len() > MAX_PDF_PAGES {
        return Err("PDF exceeds the 200-page safety limit".to_string());
    }
    let page_numbers = pages.keys().copied().collect::<Vec<_>>();
    let text = document
        .extract_text(&page_numbers)
        .map_err(|_| "PDF text extraction failed".to_string())?;
    if text.trim().is_empty() {
        return Err("PDF has no text layer; OCR is not supported yet".to_string());
    }
    if text.len() > MAX_TEXT_BYTES {
        return Err("PDF extracted text exceeds the 2 MB limit".to_string());
    }
    Ok(text)
}

fn normalize_extracted_text(content: String) -> Result<String, String> {
    let content = content.replace("\r\n", "\n").replace('\r', "\n");
    if content.trim().is_empty() {
        return Err("document contains no readable text".to_string());
    }
    if content.len() > MAX_TEXT_BYTES {
        return Err("extracted text exceeds the 2 MB limit".to_string());
    }
    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn docx(xml: &str) -> Vec<u8> {
        let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
        z.start_file(
            "word/document.xml",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        z.write_all(xml.as_bytes()).unwrap();
        z.finish().unwrap().into_inner()
    }
    #[test]
    fn invalid_utf8_and_mislabeled_files_are_rejected() {
        assert!(parse("bad.txt", &[0xff, 0xfe, 0x00]).is_err());
        assert!(parse("fake.pdf", b"plain text").is_err());
        assert!(parse("fake.txt", b"%PDF-1.7\n").is_err());
    }
    #[test]
    fn empty_and_overlarge_text_are_rejected_without_truncation() {
        assert!(parse("empty.md", b"  \n").is_err());
        assert!(parse("large.txt", &vec![b'x'; MAX_TEXT_BYTES + 1]).is_err());
        assert_eq!(
            parse("indent.md", b"    indented code\r\n")
                .unwrap()
                .content,
            "    indented code\n"
        );
    }
    #[test]
    fn docx_dtd_is_rejected_and_is_not_resolved() {
        let xml="<!DOCTYPE document [<!ENTITY x SYSTEM 'file:///etc/passwd'>]><document><p><t>&x;</t></p></document>";
        assert!(parse("bad.docx", &docx(xml)).unwrap_err().contains("DTD"));
    }
    #[test]
    fn excessive_pdf_page_count_is_rejected_before_extraction() {
        use lopdf::dictionary;
        let mut d = Document::with_version("1.5");
        let pages = d.new_object_id();
        let kids:Vec<lopdf::Object>=(0..201).map(|_|{let page=d.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),100.into(),100.into()]});page.into()}).collect();
        d.objects.insert(
            pages,
            dictionary! {"Type"=>"Pages","Kids"=>kids,"Count"=>201}.into(),
        );
        let catalog = d.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
        d.trailer.set("Root", catalog);
        let mut bytes = vec![];
        d.save_to(&mut bytes).unwrap();
        assert!(parse("many.pdf", &bytes).unwrap_err().contains("200-page"));
    }
}

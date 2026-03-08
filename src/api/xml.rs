use quick_xml::Writer;
use quick_xml::escape::escape;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use std::io::Cursor;
use uuid::Uuid;

pub fn error_response(
    code: &str,
    message: &str,
    resource: Option<&str>,
    request_id: &str,
) -> Result<String, std::io::Error> {
    let mut writer = Writer::new(Cursor::new(Vec::with_capacity(512)));

    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
    writer.write_event(Event::Start(BytesStart::new("Error")))?;

    write_element(&mut writer, "Code", code)?;
    write_element(&mut writer, "Message", message)?;
    if let Some(resource) = resource {
        write_element(&mut writer, "Resource", resource)?;
    }
    write_element(&mut writer, "RequestId", request_id)?;

    writer.write_event(Event::End(BytesEnd::new("Error")))?;

    String::from_utf8(writer.into_inner().into_inner())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

pub fn new_request_id() -> String {
    Uuid::new_v4().to_string()
}

pub fn write_element(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    name: &str,
    value: &str,
) -> Result<(), std::io::Error> {
    let escaped = escape(value);
    writer.write_event(Event::Start(BytesStart::new(name)))?;
    writer.write_event(Event::Text(BytesText::from_escaped(escaped)))?;
    writer.write_event(Event::End(BytesEnd::new(name)))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_response_produces_valid_xml() {
        let xml = error_response("NoSuchBucket", "Bucket not found", None, "req-123").unwrap();
        assert!(xml.contains("<Code>NoSuchBucket</Code>"));
        assert!(xml.contains("<Message>Bucket not found</Message>"));
        assert!(xml.contains("<RequestId>req-123</RequestId>"));
    }

    #[test]
    fn error_response_escapes_xml_special_chars() {
        let xml = error_response(
            "InvalidBucketName",
            "<script>alert('xss')</script>",
            Some("bucket&key<>"),
            "req-456",
        )
        .unwrap();
        assert!(!xml.contains("<script>"));
        assert!(xml.contains("&lt;script&gt;"));
        assert!(xml.contains("bucket&amp;key&lt;&gt;"));
    }

    #[test]
    fn new_request_id_returns_valid_uuid() {
        let id = new_request_id();
        assert!(uuid::Uuid::parse_str(&id).is_ok());
    }
}

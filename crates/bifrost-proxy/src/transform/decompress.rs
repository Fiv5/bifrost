use bytes::Bytes;
use flate2::read::{DeflateDecoder, GzDecoder, ZlibDecoder};
use std::io::Read;

pub fn decompress_body(data: &[u8], content_encoding: Option<&str>) -> Bytes {
    let encoding = match content_encoding {
        Some(e) => e.to_lowercase(),
        None => return Bytes::copy_from_slice(data),
    };

    let result = match encoding.as_str() {
        "gzip" => decompress_gzip(data),
        "deflate" => decompress_deflate(data),
        "br" => decompress_brotli(data),
        "zstd" => decompress_zstd(data),
        _ => return Bytes::copy_from_slice(data),
    };

    match result {
        Ok(decompressed) => Bytes::from(decompressed),
        Err(e) => {
            tracing::debug!("Failed to decompress {} body: {}", encoding, e);
            Bytes::copy_from_slice(data)
        }
    }
}

fn decompress_gzip(data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    let mut decoder = GzDecoder::new(data);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;
    Ok(decompressed)
}

fn decompress_deflate(data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    if let Ok(result) = decompress_zlib(data) {
        return Ok(result);
    }
    let mut decoder = DeflateDecoder::new(data);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;
    Ok(decompressed)
}

fn decompress_zlib(data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    let mut decoder = ZlibDecoder::new(data);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;
    Ok(decompressed)
}

fn decompress_brotli(data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    let mut decompressed = Vec::new();
    brotli::BrotliDecompress(&mut std::io::Cursor::new(data), &mut decompressed)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    Ok(decompressed)
}

fn decompress_zstd(data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    zstd::stream::decode_all(std::io::Cursor::new(data))
}

pub fn get_content_encoding(headers: &[(String, String)]) -> Option<String> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-encoding"))
        .map(|(_, v)| v.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decompress_no_encoding() {
        let data = b"hello world";
        let result = decompress_body(data, None);
        assert_eq!(result.as_ref(), data);
    }

    #[test]
    fn test_decompress_identity() {
        let data = b"hello world";
        let result = decompress_body(data, Some("identity"));
        assert_eq!(result.as_ref(), data);
    }

    #[test]
    fn test_decompress_gzip() {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;

        let original = b"hello world";
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(original).unwrap();
        let compressed = encoder.finish().unwrap();

        let result = decompress_body(&compressed, Some("gzip"));
        assert_eq!(result.as_ref(), original);
    }

    #[test]
    fn test_decompress_deflate() {
        use flate2::write::DeflateEncoder;
        use flate2::Compression;
        use std::io::Write;

        let original = b"hello world";
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(original).unwrap();
        let compressed = encoder.finish().unwrap();

        let result = decompress_body(&compressed, Some("deflate"));
        assert_eq!(result.as_ref(), original);
    }
}

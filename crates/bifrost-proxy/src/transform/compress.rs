use flate2::write::{GzEncoder, ZlibEncoder};
use flate2::Compression;
use std::io::Write;

pub fn compress_body(data: &[u8], content_encoding: &str) -> std::io::Result<Vec<u8>> {
    let encoding = content_encoding
        .split(',')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();

    match encoding.as_str() {
        "" | "identity" => Ok(data.to_vec()),
        "gzip" => {
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(data)?;
            encoder.finish()
        }
        // Most clients interpret "deflate" as zlib wrapped stream.
        "deflate" => {
            let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(data)?;
            encoder.finish()
        }
        "br" => {
            let mut out = Vec::new();
            {
                let mut encoder = brotli::CompressorWriter::new(&mut out, 4096, 5, 22);
                encoder.write_all(data)?;
            }
            Ok(out)
        }
        "zstd" => zstd::stream::encode_all(std::io::Cursor::new(data), 0),
        _ => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("unsupported content-encoding: {}", content_encoding),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::decompress::try_decompress_body_with_limit;

    #[test]
    fn test_gzip_roundtrip() {
        let original = b"<html><body>Hello</body></html>";
        let compressed = compress_body(original, "gzip").unwrap();
        let decompressed = try_decompress_body_with_limit(&compressed, "gzip", 1024).unwrap();
        assert_eq!(decompressed, original);
    }
}

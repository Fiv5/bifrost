use bytes::Bytes;
use flate2::read::{DeflateDecoder, GzDecoder, ZlibDecoder};
use std::io::{Read, Write};

const DEFAULT_MAX_DECOMPRESS_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

pub fn decompress_body(data: &[u8], content_encoding: Option<&str>) -> Bytes {
    decompress_body_with_limit(data, content_encoding, DEFAULT_MAX_DECOMPRESS_OUTPUT_BYTES)
}

/// 解压 HTTP body（gzip/deflate/br/zstd），并限制解压后的最大输出大小。
///
/// - 当 `max_output_bytes` 为 0 时，直接返回原始数据。
/// - 当解压输出超过上限时，放弃解压并回退到原始压缩数据（用于防止压缩炸弹）。
pub fn decompress_body_with_limit(
    data: &[u8],
    content_encoding: Option<&str>,
    max_output_bytes: usize,
) -> Bytes {
    if max_output_bytes == 0 {
        return Bytes::copy_from_slice(data);
    }

    let encoding = match content_encoding {
        Some(e) => e.to_lowercase(),
        None => return Bytes::copy_from_slice(data),
    };

    let result = match encoding.as_str() {
        "gzip" => decompress_gzip_limited(data, max_output_bytes),
        "deflate" => decompress_deflate_limited(data, max_output_bytes),
        "br" => decompress_brotli_limited(data, max_output_bytes),
        "zstd" => decompress_zstd_limited(data, max_output_bytes),
        _ => return Bytes::copy_from_slice(data),
    };

    match result {
        Ok(decompressed) => Bytes::from(decompressed),
        Err(e) => {
            tracing::debug!(
                "Failed to decompress {} body (limit={}): {}",
                encoding,
                max_output_bytes,
                e
            );
            Bytes::copy_from_slice(data)
        }
    }
}

fn decompress_gzip_limited(
    data: &[u8],
    max_output_bytes: usize,
) -> Result<Vec<u8>, std::io::Error> {
    let mut decoder = GzDecoder::new(data);
    read_to_end_limited(&mut decoder, max_output_bytes)
}

fn decompress_deflate_limited(
    data: &[u8],
    max_output_bytes: usize,
) -> Result<Vec<u8>, std::io::Error> {
    if let Ok(result) = decompress_zlib_limited(data, max_output_bytes) {
        return Ok(result);
    }
    let mut decoder = DeflateDecoder::new(data);
    read_to_end_limited(&mut decoder, max_output_bytes)
}

fn decompress_zlib_limited(
    data: &[u8],
    max_output_bytes: usize,
) -> Result<Vec<u8>, std::io::Error> {
    let mut decoder = ZlibDecoder::new(data);
    read_to_end_limited(&mut decoder, max_output_bytes)
}

fn decompress_brotli_limited(
    data: &[u8],
    max_output_bytes: usize,
) -> Result<Vec<u8>, std::io::Error> {
    let mut decompressed = Vec::new();
    let mut writer = LimitedWriter::new(&mut decompressed, max_output_bytes);
    brotli::BrotliDecompress(&mut std::io::Cursor::new(data), &mut writer)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    Ok(decompressed)
}

fn decompress_zstd_limited(
    data: &[u8],
    max_output_bytes: usize,
) -> Result<Vec<u8>, std::io::Error> {
    let cursor = std::io::Cursor::new(data);
    let mut decoder = zstd::stream::read::Decoder::new(cursor)?;
    read_to_end_limited(&mut decoder, max_output_bytes)
}

fn read_to_end_limited<R: Read>(
    reader: &mut R,
    max_output_bytes: usize,
) -> Result<Vec<u8>, std::io::Error> {
    let mut limited = reader.take((max_output_bytes as u64).saturating_add(1));
    let mut out = Vec::new();
    limited.read_to_end(&mut out)?;
    if out.len() > max_output_bytes {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "decompressed body too large ({} > {} bytes)",
                out.len(),
                max_output_bytes
            ),
        ));
    }
    Ok(out)
}

struct LimitedWriter<'a> {
    inner: &'a mut Vec<u8>,
    limit: usize,
}

impl<'a> LimitedWriter<'a> {
    fn new(inner: &'a mut Vec<u8>, limit: usize) -> Self {
        Self { inner, limit }
    }
}

impl Write for LimitedWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let next_len = self.inner.len().saturating_add(buf.len());
        if next_len > self.limit {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "decompressed body too large ({} > {} bytes)",
                    next_len, self.limit
                ),
            ));
        }
        self.inner.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
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

mod deflate;
mod forwarder;
mod frame;
mod handshake;
mod io;

pub use deflate::*;
pub use forwarder::*;
pub use frame::*;
pub use handshake::*;
pub use io::*;

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::{BufMut, Bytes, BytesMut};
    use flate2::{Compress, Compression, FlushCompress};

    #[test]
    fn test_opcode_from_u8() {
        assert_eq!(Opcode::from_u8(0x0), Some(Opcode::Continuation));
        assert_eq!(Opcode::from_u8(0x1), Some(Opcode::Text));
        assert_eq!(Opcode::from_u8(0x2), Some(Opcode::Binary));
        assert_eq!(Opcode::from_u8(0x8), Some(Opcode::Close));
        assert_eq!(Opcode::from_u8(0x9), Some(Opcode::Ping));
        assert_eq!(Opcode::from_u8(0xA), Some(Opcode::Pong));
        assert_eq!(Opcode::from_u8(0x3), None);
    }

    #[test]
    fn test_opcode_is_control() {
        assert!(!Opcode::Text.is_control());
        assert!(!Opcode::Binary.is_control());
        assert!(!Opcode::Continuation.is_control());
        assert!(Opcode::Close.is_control());
        assert!(Opcode::Ping.is_control());
        assert!(Opcode::Pong.is_control());
    }

    #[test]
    fn test_frame_encode_decode() {
        let frame = WebSocketFrame::text("Hello, World!");
        let encoded = frame.encode();
        let (decoded, consumed) = WebSocketFrame::parse(&encoded).unwrap();

        assert!(decoded.fin);
        assert_eq!(decoded.opcode, Opcode::Text);
        assert_eq!(decoded.payload.as_ref(), b"Hello, World!");
        assert_eq!(consumed, encoded.len());
    }

    #[test]
    fn test_frame_encode_decode_with_mask() {
        let mask = [0x12, 0x34, 0x56, 0x78];
        let frame = WebSocketFrame::text("Hello").with_mask(mask);
        let encoded = frame.encode();
        let (decoded, consumed) = WebSocketFrame::parse(&encoded).unwrap();

        assert_eq!(decoded.payload.as_ref(), b"Hello");
        assert_eq!(consumed, encoded.len());
    }

    #[test]
    fn test_frame_encode_large_payload() {
        let data = vec![0u8; 1000];
        let frame = WebSocketFrame::binary(&data);
        let encoded = frame.encode();
        let (decoded, _) = WebSocketFrame::parse(&encoded).unwrap();

        assert_eq!(decoded.payload.len(), 1000);
    }

    #[test]
    fn test_frame_close() {
        let frame = WebSocketFrame::close(Some(1000), "Normal closure");
        assert_eq!(frame.close_code(), Some(1000));
        assert_eq!(frame.close_reason(), Some("Normal closure"));
    }

    #[test]
    fn test_frame_ping_pong() {
        let ping = WebSocketFrame::ping(b"test");
        assert_eq!(ping.opcode, Opcode::Ping);
        assert_eq!(ping.payload.as_ref(), b"test");

        let pong = WebSocketFrame::pong(b"test");
        assert_eq!(pong.opcode, Opcode::Pong);
        assert_eq!(pong.payload.as_ref(), b"test");
    }

    #[test]
    fn test_compute_accept_key() {
        let key = "dGhlIHNhbXBsZSBub25jZQ==";
        let accept = compute_accept_key(key);
        assert_eq!(accept, "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
    }

    #[test]
    fn test_generate_sec_websocket_key() {
        let key = generate_sec_websocket_key();
        assert!(!key.is_empty());
    }

    #[test]
    fn test_build_websocket_request_headers() {
        let headers =
            build_websocket_request_headers("example.com", "/ws", "dGVzdA==", Some(&["chat"]));

        assert!(headers.contains("GET /ws HTTP/1.1"));
        assert!(headers.contains("Host: example.com"));
        assert!(headers.contains("Upgrade: websocket"));
        assert!(headers.contains("Connection: Upgrade"));
        assert!(headers.contains("Sec-WebSocket-Key: dGVzdA=="));
        assert!(headers.contains("Sec-WebSocket-Version: 13"));
        assert!(headers.contains("Sec-WebSocket-Protocol: chat"));
    }

    #[test]
    fn test_build_websocket_response_headers() {
        let headers = build_websocket_response_headers("dGVzdA==", Some("chat"));

        assert!(headers.contains("HTTP/1.1 101 Switching Protocols"));
        assert!(headers.contains("Upgrade: websocket"));
        assert!(headers.contains("Connection: Upgrade"));
        assert!(headers.contains("Sec-WebSocket-Accept:"));
        assert!(headers.contains("Sec-WebSocket-Protocol: chat"));
    }

    #[test]
    fn test_frame_parse_incomplete() {
        let data = [0x81]; // Only first byte
        assert!(WebSocketFrame::parse(&data).is_none());
    }

    #[test]
    fn test_frame_parse_incomplete_payload() {
        let data = [0x81, 0x05, 0x48]; // Text frame with 5 bytes, but only 1 byte payload
        assert!(WebSocketFrame::parse(&data).is_none());
    }

    #[test]
    fn test_frame_rsv1_parsing() {
        let mut buf = BytesMut::new();
        buf.put_u8(0xC1);
        buf.put_u8(0x05);
        buf.extend_from_slice(b"hello");

        let (frame, _) = WebSocketFrame::parse(&buf).unwrap();
        assert!(frame.fin);
        assert!(frame.rsv1);
        assert_eq!(frame.opcode, Opcode::Text);
        assert!(frame.is_compressed());
    }

    #[test]
    fn test_frame_rsv1_encoding() {
        let frame = WebSocketFrame {
            fin: true,
            rsv1: true,
            rsv2: false,
            rsv3: false,
            opcode: Opcode::Text,
            mask: None,
            payload: Bytes::from("hello"),
        };
        let encoded = frame.encode();
        assert_eq!(encoded[0], 0xC1);
    }

    #[test]
    fn test_frame_rsv2_rsv3_roundtrip() {
        let frame = WebSocketFrame {
            fin: true,
            rsv1: false,
            rsv2: true,
            rsv3: true,
            opcode: Opcode::Binary,
            mask: None,
            payload: Bytes::from_static(b"hi"),
        };
        let encoded = frame.encode();
        let (decoded, consumed) = WebSocketFrame::parse(&encoded).unwrap();
        assert_eq!(consumed, encoded.len());
        assert!(decoded.rsv2);
        assert!(decoded.rsv3);
        assert_eq!(decoded.payload.as_ref(), b"hi");
    }

    #[test]
    fn test_parse_permessage_deflate() {
        assert!(parse_permessage_deflate("permessage-deflate"));
        assert!(parse_permessage_deflate(
            "permessage-deflate; client_max_window_bits"
        ));
        assert!(parse_permessage_deflate(
            "permessage-deflate; server_no_context_takeover"
        ));
        assert!(!parse_permessage_deflate("x-webkit-deflate-frame"));
        assert!(!parse_permessage_deflate(""));
    }

    #[test]
    fn test_parse_permessage_deflate_config_params() {
        let cfg = parse_permessage_deflate_config(
            "permessage-deflate; server_no_context_takeover; client_no_context_takeover; client_max_window_bits=15; server_max_window_bits=10",
        )
        .unwrap();

        assert!(cfg.enabled());
        assert!(cfg.server_no_context_takeover);
        assert!(cfg.client_no_context_takeover);
        assert_eq!(cfg.client_max_window_bits, Some(15));
        assert_eq!(cfg.server_max_window_bits, Some(10));

        let cfg2 =
            parse_permessage_deflate_config("foo, permessage-deflate; client_max_window_bits; bar")
                .unwrap();
        assert!(cfg2.enabled());
    }

    fn compress_permessage_deflate_takeover(compressor: &mut Compress, input: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut buf = [0u8; 8192];

        // 把 input 压缩并做一次 sync flush
        let mut input_pos = 0usize;
        while input_pos < input.len() {
            let before_in = compressor.total_in();
            let before_out = compressor.total_out();

            let _ = compressor
                .compress(&input[input_pos..], &mut buf, FlushCompress::Sync)
                .unwrap();

            let used_in = (compressor.total_in() - before_in) as usize;
            let produced_out = (compressor.total_out() - before_out) as usize;
            if produced_out > 0 {
                out.extend_from_slice(&buf[..produced_out]);
            }
            input_pos = input_pos.saturating_add(used_in);
            if used_in == 0 && produced_out == 0 {
                break;
            }
        }

        // 对空输入再 flush 一次，把剩余输出取出来（避免长循环卡住）
        let before_out = compressor.total_out();
        let _ = compressor
            .compress(&[], &mut buf, FlushCompress::Sync)
            .unwrap();
        let produced_out = (compressor.total_out() - before_out) as usize;
        if produced_out > 0 {
            out.extend_from_slice(&buf[..produced_out]);
        }

        // permessage-deflate：去掉结尾 0x00 0x00 0xff 0xff
        if out.len() >= 4 && out[out.len() - 4..] == [0x00, 0x00, 0xff, 0xff] {
            out.truncate(out.len() - 4);
        }
        out
    }

    #[test]
    fn test_permessage_deflate_context_takeover_inflater() {
        let msg1 = b"hello hello hello hello hello";
        let msg2 = b"world hello hello hello world";

        // 模拟 context takeover：复用同一个 compressor
        let mut compressor = Compress::new(Compression::default(), false);
        let c1 = compress_permessage_deflate_takeover(&mut compressor, msg1);
        let c2 = compress_permessage_deflate_takeover(&mut compressor, msg2);

        // 正确解法：同一个 inflater 连续解两条消息
        let mut inflater = PerMessageDeflateInflater::new();
        let d1 = inflater.decompress_message(&c1).unwrap();
        let d2 = inflater.decompress_message(&c2).unwrap();
        assert_eq!(d1.as_ref(), msg1);
        assert_eq!(d2.as_ref(), msg2);

        // 旧行为（每条消息新建 inflater）：第二条消息往往无法正确解压
        let mut fresh = PerMessageDeflateInflater::new();
        let d2_bad = fresh.decompress_message(&c2);
        assert!(d2_bad.is_err() || d2_bad.unwrap().as_ref() != msg2);
    }

    #[test]
    fn test_extract_sec_websocket_extensions() {
        let response = "HTTP/1.1 101 Switching Protocols\r\n\
                        Upgrade: websocket\r\n\
                        Connection: Upgrade\r\n\
                        Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\
                        Sec-WebSocket-Extensions: permessage-deflate; server_no_context_takeover\r\n\r\n";
        let ext = extract_sec_websocket_extensions(response);
        assert!(ext.is_some());
        assert!(ext.unwrap().contains("permessage-deflate"));
    }

    #[test]
    fn test_extract_sec_websocket_extensions_missing() {
        let response = "HTTP/1.1 101 Switching Protocols\r\n\
                        Upgrade: websocket\r\n\
                        Connection: Upgrade\r\n\r\n";
        let ext = extract_sec_websocket_extensions(response);
        assert!(ext.is_none());
    }

    #[test]
    fn test_decompress_payload_uncompressed() {
        let frame = WebSocketFrame {
            fin: true,
            rsv1: false,
            rsv2: false,
            rsv3: false,
            opcode: Opcode::Text,
            mask: None,
            payload: Bytes::from("hello"),
        };
        let decompressed = frame.decompress_payload();
        assert_eq!(decompressed.as_ref(), b"hello");
    }

    #[test]
    fn test_decompress_payload_compressed() {
        use flate2::write::DeflateEncoder;
        use flate2::Compression;
        use std::io::Write;

        let original = b"hello world hello world";
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(original).unwrap();
        let compressed = encoder.finish().unwrap();

        let frame = WebSocketFrame {
            fin: true,
            rsv1: true,
            rsv2: false,
            rsv3: false,
            opcode: Opcode::Text,
            mask: None,
            payload: Bytes::from(compressed),
        };

        let decompressed = frame.decompress_payload();
        assert_eq!(decompressed.as_ref(), original);
    }

    #[test]
    fn test_decompress_payload_rfc7692_format() {
        use flate2::write::DeflateEncoder;
        use flate2::Compression;
        use std::io::Write;

        let original = b"Hello";
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(original).unwrap();
        let mut compressed = encoder.finish().unwrap();

        if compressed.ends_with(&[0x00, 0x00, 0xff, 0xff]) {
            compressed.truncate(compressed.len() - 4);
        }

        let frame = WebSocketFrame {
            fin: true,
            rsv1: true,
            rsv2: false,
            rsv3: false,
            opcode: Opcode::Text,
            mask: None,
            payload: Bytes::from(compressed),
        };

        let decompressed = frame.decompress_payload();
        assert_eq!(decompressed.as_ref(), original);
    }
}

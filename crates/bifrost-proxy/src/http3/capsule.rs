use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::io::{self, Cursor};

pub const DATAGRAM_CAPSULE_TYPE: u64 = 0x00;
pub const CLOSE_WEBTRANSPORT_SESSION_TYPE: u64 = 0x2843;
pub const DRAIN_WEBTRANSPORT_SESSION_TYPE: u64 = 0x78ae;

pub const ADDRESS_ASSIGN_CAPSULE_TYPE: u64 = 0x01;
pub const ADDRESS_REQUEST_CAPSULE_TYPE: u64 = 0x02;
pub const ROUTE_ADVERTISEMENT_CAPSULE_TYPE: u64 = 0x03;

#[derive(Debug, Clone, PartialEq)]
pub enum CapsuleType {
    Datagram,
    CloseWebTransportSession,
    DrainWebTransportSession,
    AddressAssign,
    AddressRequest,
    RouteAdvertisement,
    Unknown(u64),
}

impl From<u64> for CapsuleType {
    fn from(value: u64) -> Self {
        match value {
            DATAGRAM_CAPSULE_TYPE => CapsuleType::Datagram,
            CLOSE_WEBTRANSPORT_SESSION_TYPE => CapsuleType::CloseWebTransportSession,
            DRAIN_WEBTRANSPORT_SESSION_TYPE => CapsuleType::DrainWebTransportSession,
            ADDRESS_ASSIGN_CAPSULE_TYPE => CapsuleType::AddressAssign,
            ADDRESS_REQUEST_CAPSULE_TYPE => CapsuleType::AddressRequest,
            ROUTE_ADVERTISEMENT_CAPSULE_TYPE => CapsuleType::RouteAdvertisement,
            other => CapsuleType::Unknown(other),
        }
    }
}

impl From<CapsuleType> for u64 {
    fn from(value: CapsuleType) -> Self {
        match value {
            CapsuleType::Datagram => DATAGRAM_CAPSULE_TYPE,
            CapsuleType::CloseWebTransportSession => CLOSE_WEBTRANSPORT_SESSION_TYPE,
            CapsuleType::DrainWebTransportSession => DRAIN_WEBTRANSPORT_SESSION_TYPE,
            CapsuleType::AddressAssign => ADDRESS_ASSIGN_CAPSULE_TYPE,
            CapsuleType::AddressRequest => ADDRESS_REQUEST_CAPSULE_TYPE,
            CapsuleType::RouteAdvertisement => ROUTE_ADVERTISEMENT_CAPSULE_TYPE,
            CapsuleType::Unknown(v) => v,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Capsule {
    pub capsule_type: CapsuleType,
    pub data: Bytes,
}

impl Capsule {
    pub fn new(capsule_type: CapsuleType, data: Bytes) -> Self {
        Self { capsule_type, data }
    }

    pub fn datagram(context_id: u64, payload: Bytes) -> Self {
        let mut data = BytesMut::new();
        encode_varint(context_id, &mut data);
        data.extend_from_slice(&payload);
        Self::new(CapsuleType::Datagram, data.freeze())
    }

    pub fn encode(&self) -> Bytes {
        let mut buf = BytesMut::new();
        let type_val: u64 = self.capsule_type.clone().into();
        encode_varint(type_val, &mut buf);
        encode_varint(self.data.len() as u64, &mut buf);
        buf.extend_from_slice(&self.data);
        buf.freeze()
    }

    pub fn decode(data: &mut impl Buf) -> io::Result<Option<Self>> {
        if data.remaining() < 2 {
            return Ok(None);
        }

        let capsule_type = match decode_varint(data) {
            Some(v) => CapsuleType::from(v),
            None => return Ok(None),
        };

        let length = match decode_varint(data) {
            Some(v) => v as usize,
            None => return Ok(None),
        };

        if data.remaining() < length {
            return Ok(None);
        }

        let payload = data.copy_to_bytes(length);

        Ok(Some(Capsule {
            capsule_type,
            data: payload,
        }))
    }

    pub fn parse_datagram_payload(&self) -> io::Result<(u64, Bytes)> {
        if self.capsule_type != CapsuleType::Datagram {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Not a datagram capsule",
            ));
        }

        let mut cursor = Cursor::new(&self.data[..]);
        let context_id = decode_varint(&mut cursor)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Invalid context ID"))?;

        let pos = cursor.position() as usize;
        let payload = self.data.slice(pos..);

        Ok((context_id, payload))
    }
}

pub fn encode_varint(value: u64, buf: &mut BytesMut) {
    if value < 64 {
        buf.put_u8(value as u8);
    } else if value < 16384 {
        buf.put_u8(0x40 | ((value >> 8) as u8));
        buf.put_u8(value as u8);
    } else if value < 1073741824 {
        buf.put_u8(0x80 | ((value >> 24) as u8));
        buf.put_u8((value >> 16) as u8);
        buf.put_u8((value >> 8) as u8);
        buf.put_u8(value as u8);
    } else {
        buf.put_u8(0xc0 | ((value >> 56) as u8));
        buf.put_u8((value >> 48) as u8);
        buf.put_u8((value >> 40) as u8);
        buf.put_u8((value >> 32) as u8);
        buf.put_u8((value >> 24) as u8);
        buf.put_u8((value >> 16) as u8);
        buf.put_u8((value >> 8) as u8);
        buf.put_u8(value as u8);
    }
}

pub fn decode_varint(buf: &mut impl Buf) -> Option<u64> {
    if buf.remaining() < 1 {
        return None;
    }

    let first = buf.get_u8();
    let prefix = first >> 6;

    match prefix {
        0 => Some(first as u64),
        1 => {
            if buf.remaining() < 1 {
                return None;
            }
            let second = buf.get_u8();
            Some((((first & 0x3f) as u64) << 8) | (second as u64))
        }
        2 => {
            if buf.remaining() < 3 {
                return None;
            }
            let b1 = buf.get_u8();
            let b2 = buf.get_u8();
            let b3 = buf.get_u8();
            Some(
                (((first & 0x3f) as u64) << 24)
                    | ((b1 as u64) << 16)
                    | ((b2 as u64) << 8)
                    | (b3 as u64),
            )
        }
        3 => {
            if buf.remaining() < 7 {
                return None;
            }
            let mut value = ((first & 0x3f) as u64) << 56;
            for i in (0..7).rev() {
                value |= (buf.get_u8() as u64) << (i * 8);
            }
            Some(value)
        }
        _ => unreachable!(),
    }
}

#[derive(Debug, Clone)]
pub struct UdpProxyHeader {
    pub context_id: u64,
}

impl UdpProxyHeader {
    pub fn new(context_id: u64) -> Self {
        Self { context_id }
    }

    pub fn encode(&self) -> Bytes {
        let mut buf = BytesMut::new();
        encode_varint(self.context_id, &mut buf);
        buf.freeze()
    }

    pub fn decode(data: &mut impl Buf) -> io::Result<Option<Self>> {
        let context_id = match decode_varint(data) {
            Some(v) => v,
            None => return Ok(None),
        };

        Ok(Some(Self { context_id }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_varint_encode_decode() {
        let test_cases = vec![
            0u64,
            1,
            63,
            64,
            16383,
            16384,
            1073741823,
            1073741824,
            u64::MAX >> 2,
        ];

        for value in test_cases {
            let mut buf = BytesMut::new();
            encode_varint(value, &mut buf);
            let decoded = decode_varint(&mut buf.freeze()).unwrap();
            assert_eq!(value, decoded, "Failed for value {}", value);
        }
    }

    #[test]
    fn test_capsule_encode_decode() {
        let original = Capsule::datagram(0, Bytes::from_static(b"hello world"));
        let encoded = original.encode();
        let mut cursor = Cursor::new(encoded);
        let decoded = Capsule::decode(&mut cursor).unwrap().unwrap();

        assert_eq!(decoded.capsule_type, CapsuleType::Datagram);
        let (context_id, payload) = decoded.parse_datagram_payload().unwrap();
        assert_eq!(context_id, 0);
        assert_eq!(payload.as_ref(), b"hello world");
    }

    #[test]
    fn test_udp_proxy_header() {
        let header = UdpProxyHeader::new(12345);
        let encoded = header.encode();
        let mut cursor = Cursor::new(encoded);
        let decoded = UdpProxyHeader::decode(&mut cursor).unwrap().unwrap();
        assert_eq!(decoded.context_id, 12345);
    }
}

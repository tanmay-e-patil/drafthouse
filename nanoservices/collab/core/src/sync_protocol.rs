use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
/// Y-websocket binary protocol helpers.
///
/// Message format (all values are unsigned varint / length-prefixed):
///   Sync  (type 0): [0, step, <payload>]  step: 0=SyncStep1, 1=SyncStep2, 2=Update
///   Awareness (type 1): [1, <payload>]
///   Custom (type 3): [3, <payload>]  — used for title_update
///
/// Payload is a varint-prefixed byte array (lib0 encoding).
use yrs::{Doc, ReadTxn, StateVector, Transact, Update};

const MSG_SYNC: u8 = 0;
const SYNC_STEP1: u8 = 0;
const SYNC_STEP2: u8 = 1;
const SYNC_UPDATE: u8 = 2;

pub enum CollabMessage {
    SyncStep1(Vec<u8>),
    SyncStep2(Vec<u8>),
    Update(Vec<u8>),
    Awareness(Vec<u8>),
    Unknown,
}

fn write_varint(buf: &mut Vec<u8>, mut n: usize) {
    loop {
        let b = (n & 0x7F) as u8;
        n >>= 7;
        if n == 0 {
            buf.push(b);
            break;
        } else {
            buf.push(b | 0x80);
        }
    }
}

fn write_bytes(buf: &mut Vec<u8>, data: &[u8]) {
    write_varint(buf, data.len());
    buf.extend_from_slice(data);
}

fn read_varint(data: &[u8], pos: &mut usize) -> Option<usize> {
    let mut result = 0usize;
    let mut shift = 0;
    loop {
        if *pos >= data.len() {
            return None;
        }
        let b = data[*pos] as usize;
        *pos += 1;
        result |= (b & 0x7F) << shift;
        if b & 0x80 == 0 {
            return Some(result);
        }
        shift += 7;
        if shift >= 64 {
            return None;
        }
    }
}

fn read_bytes<'a>(data: &'a [u8], pos: &mut usize) -> Option<&'a [u8]> {
    let len = read_varint(data, pos)?;
    if *pos + len > data.len() {
        return None;
    }
    let slice = &data[*pos..*pos + len];
    *pos += len;
    Some(slice)
}

pub fn decode_message(data: &[u8]) -> CollabMessage {
    if data.is_empty() {
        return CollabMessage::Unknown;
    }
    let mut pos = 0;
    let msg_type = data[pos] as usize;
    pos += 1;

    match msg_type {
        0 => {
            // Sync
            if pos >= data.len() {
                return CollabMessage::Unknown;
            }
            let step = data[pos] as usize;
            pos += 1;
            match read_bytes(data, &mut pos) {
                Some(payload) => match step {
                    0 => CollabMessage::SyncStep1(payload.to_vec()),
                    1 => CollabMessage::SyncStep2(payload.to_vec()),
                    2 => CollabMessage::Update(payload.to_vec()),
                    _ => CollabMessage::Unknown,
                },
                None => CollabMessage::Unknown,
            }
        }
        1 => {
            // Awareness
            match read_bytes(data, &mut pos) {
                Some(payload) => CollabMessage::Awareness(payload.to_vec()),
                None => CollabMessage::Unknown,
            }
        }
        _ => CollabMessage::Unknown,
    }
}

/// Encode a SyncStep1 message containing the doc's current state vector.
pub fn encode_sync_step1(doc: &Doc) -> Vec<u8> {
    let txn = doc.transact();
    let sv = txn.state_vector();
    let sv_bytes = sv.encode_v1();
    let mut buf = vec![MSG_SYNC, SYNC_STEP1];
    write_bytes(&mut buf, &sv_bytes);
    buf
}

/// Encode a SyncStep2 message containing the update diff for the given state vector.
pub fn encode_sync_step2(doc: &Doc, client_sv_bytes: &[u8]) -> Vec<u8> {
    let client_sv = StateVector::decode_v1(client_sv_bytes).unwrap_or_default();
    let txn = doc.transact();
    let update = txn.encode_state_as_update_v1(&client_sv);
    let mut buf = vec![MSG_SYNC, SYNC_STEP2];
    write_bytes(&mut buf, &update);
    buf
}

/// Encode a full-state SyncStep2 (for new joiners who send empty state vector).
pub fn encode_full_sync_step2(doc: &Doc) -> Vec<u8> {
    let txn = doc.transact();
    let update = txn.encode_state_as_update_v1(&StateVector::default());
    let mut buf = vec![MSG_SYNC, SYNC_STEP2];
    write_bytes(&mut buf, &update);
    buf
}

/// Encode an Update broadcast message.
pub fn encode_update(update_bytes: &[u8]) -> Vec<u8> {
    let mut buf = vec![MSG_SYNC, SYNC_UPDATE];
    write_bytes(&mut buf, update_bytes);
    buf
}

/// Apply a raw update to the doc inside a catch_unwind boundary.
/// Returns the serialized update bytes on success (for WAL write + broadcast).
pub fn apply_update_safe(doc: &Doc, update_bytes: &[u8]) -> Option<Vec<u8>> {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let update = Update::decode_v1(update_bytes).ok()?;
        let mut txn = doc.transact_mut();
        txn.apply_update(update).ok()?;
        Some(update_bytes.to_vec())
    }));
    result.ok().flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use yrs::{Doc, GetString, Text, Transact};

    #[test]
    fn encode_decode_sync_step1_roundtrip() {
        let doc = Doc::new();
        let msg = encode_sync_step1(&doc);
        match decode_message(&msg) {
            CollabMessage::SyncStep1(_) => {}
            _ => panic!("expected SyncStep1"),
        }
    }

    #[test]
    fn encode_decode_update_roundtrip() {
        let payload = b"hello";
        let msg = encode_update(payload);
        match decode_message(&msg) {
            CollabMessage::Update(data) => assert_eq!(data, payload),
            _ => panic!("expected Update"),
        }
    }

    #[test]
    fn apply_update_safe_bad_bytes_returns_none() {
        let doc = Doc::new();
        let result = apply_update_safe(&doc, &[0xFF, 0xFE, 0xFD]);
        // bad bytes should not panic, just return None
        assert!(result.is_none());
    }

    #[test]
    fn two_docs_converge_after_update_exchange() {
        let doc_a = Doc::new();
        let doc_b = Doc::new();

        // A makes a change
        {
            let text = doc_a.get_or_insert_text("content");
            let mut txn = doc_a.transact_mut();
            text.insert(&mut txn, 0, "hello");
        }

        // encode A's full state as update
        let update_bytes = {
            let txn = doc_a.transact();
            txn.encode_state_as_update_v1(&StateVector::default())
        };

        // apply to B
        let applied = apply_update_safe(&doc_b, &update_bytes);
        assert!(applied.is_some());

        // both docs should have same content
        let text_b = doc_b.get_or_insert_text("content");
        let txn_b = doc_b.transact();
        assert_eq!(text_b.get_string(&txn_b), "hello");
    }
}

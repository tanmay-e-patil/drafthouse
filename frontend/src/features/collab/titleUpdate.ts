/**
 * Decodes a custom WS message of type 3 (title_update).
 *
 * Wire format: [0x03, varint(len), ...utf8_bytes]
 * Returns the title string, or null if the buffer is not a valid type-3 message.
 */
export function decodeTitleUpdate(buf: Uint8Array): string | null {
  if (buf.length < 2 || buf[0] !== 3) return null;

  let pos = 1;
  let len = 0;
  let shift = 0;
  while (pos < buf.length) {
    const b = buf[pos++];
    len |= (b & 0x7f) << shift;
    if ((b & 0x80) === 0) break;
    shift += 7;
    if (shift > 28) return null; // overflow guard
  }

  if (pos + len > buf.length) return null;
  return new TextDecoder().decode(buf.subarray(pos, pos + len));
}

/**
 * Encodes a title_update message (type 3) for testing / mock purposes.
 * Mirrors the Rust `encode_title_update` function.
 */
export function encodeTitleUpdate(title: string): Uint8Array {
  const titleBytes = new TextEncoder().encode(title);
  const lenBytes: number[] = [];
  let n = titleBytes.length;
  do {
    const b = n & 0x7f;
    n >>>= 7;
    lenBytes.push(n > 0 ? b | 0x80 : b);
  } while (n > 0);
  const out = new Uint8Array(1 + lenBytes.length + titleBytes.length);
  out[0] = 3;
  for (let i = 0; i < lenBytes.length; i++) out[1 + i] = lenBytes[i];
  out.set(titleBytes, 1 + lenBytes.length);
  return out;
}

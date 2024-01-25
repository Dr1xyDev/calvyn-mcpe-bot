pub struct Frame {
    pub reliability: u8,
    pub has_split: bool,
    pub length: u16,
    pub payload: Vec<u8>,
}

pub fn parse_frames(data: &[u8], count: u32) -> Vec<Frame> {
    let mut frames = Vec::new();
    let mut off = 0;
    for _ in 0..count {
        let flags = data.get(off).copied().unwrap_or(0);
        off += 1;
        let reliability = flags >> 5;
        let has_split = (flags & 0x10) != 0;
        let length = {
            let mut b = [0u8; 2];
            b.copy_from_slice(&data[off..off+2]);
            u16::from_be_bytes(b)
        };
        off += 2;
        let mut skip = match reliability {
            0 | 2 => 0,
            1 | 4 => 8,
            3 => 4,
            _ => 0,
        };
        if has_split {
            skip += 10;
        }
        off += skip as usize;
        let payload = data[off..off + length as usize].to_vec();
        off += length as usize;
        frames.push(Frame { reliability, has_split, length, payload });
    }
    frames
}

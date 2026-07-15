pub struct TextMaskEncoder {
    secret_mask: u64,
}

pub struct TextMarkDecoder {
    secret_mask: u64,
}

impl TextMaskEncoder {
    // Pick a large, random-looking prime or hex number as your secret mask
    fn new(seed: u64) -> Self {
        Self { secret_mask: seed }
    }

    pub fn encode(&self, user_id: u32, sequence: u32) -> u64 {
        // 1. Pack two 32-bit numbers into one 64-bit number
        // This prevents the collisions you'd get with multiplication
        let packed: u64 = ((user_id as u64) << 32) | (sequence as u64);

        // 2. XOR with the secret mask to scramble it
        packed ^ self.secret_mask
    }
}
impl TextMarkDecoder {
    // Pick a large, random-looking prime or hex number as your secret mask
    fn new(seed: u64) -> Self {
        Self { secret_mask: seed }
    }
    pub fn decode(&self, public_id: u64) -> (u32, u32) {
        // 1. Unscramble using the same mask
        let unpacked = public_id ^ self.secret_mask;

        // 2. Extract the numbers back out
        let user_id = (unpacked >> 32) as u32;
        let sequence = (unpacked & 0xFFFFFFFF) as u32;

        (user_id, sequence)
    }
}

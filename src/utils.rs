
pub fn get_bits(value: u64, to: usize, from: usize) -> u64 {
    let num = to + 1 - from;
    (value >> from) & ((1u64 << num) - 1)
}

pub fn get_bits_u32(value: u32, to: usize, from: usize) -> u32 {
    let num = to + 1 - from;
    (value >> from) & ((1u32 << num) - 1)
}

pub fn sign_extend(value: u32, bits: u32) -> u32 {
    let sign_bit = 1u32 << (bits - 1);
    let extended = (value & (sign_bit - 1)) as i32 - (value & sign_bit) as i32;
    extended as u32
}

pub fn u32_to_f32(val: u32) -> f32 {
    unsafe {
        std::mem::transmute::<u32, f32>(val)
    }
}

pub fn f32_to_u32(val: f32) -> u32 {
    unsafe {
        std::mem::transmute::<f32, u32>(val)
    }
}

pub fn u32_to_u8x4(val: u32) -> [u8; 4] {
    unsafe {
        std::mem::transmute::<u32, [u8; 4]>(val)
    }
}

pub fn u8x4_to_u32(val: [u8; 4]) -> u32 {
    unsafe {
        std::mem::transmute::<[u8; 4], u32>(val)
    }
}

pub fn unwrap_u32x16(values: &[Option<u32>; 16]) -> [u32; 16] {
    let mut results: [u32; 16] = Default::default();
    results.clone_from_slice(&values.iter().map(|x| { x.unwrap() }).collect::<Vec<u32>>()[..]);
    results
}

pub fn reduction_and(values: &[bool], inv: bool) -> bool {
    values.iter().fold(true, |acc: bool, value: &bool| { (inv ^ value) & acc })
}

pub fn reduction_or(values: &[bool], inv: bool) -> bool {
    values.iter().fold(false, |acc: bool, value: &bool| { (inv ^ value) | acc })
}
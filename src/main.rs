mod processor;
mod utils;
mod instructions;
mod constants;

use processor::QPUEmu;
use utils::*;

use std::fs;
use std::io::{Read};
use byteorder::{ByteOrder, LittleEndian};
use std::time::{Instant};
use rand::distributions::{Uniform, Distribution};

fn sgemm(M: usize, N: usize, K: usize, alpha: f32, A: &Vec<f32>, B: &Vec<f32>, beta: f32, C: &mut Vec<f32>) {

    for m in 0..M {
        for n in 0..N {
            let mut AB = 0.0f32;
            for k in 0..K {
                AB += A[m * K + k] * B[k * N + n];
            }
            C[m * N + n] = alpha * AB + beta * C[m * N + n];
        }
    }
}

fn read<T: std::str::FromStr>() -> T {
    let mut s = String::new();
    std::io::stdin().read_line(&mut s).ok();
    s.trim().parse().ok().unwrap()
}

fn breakpoint_handler(emu: &QPUEmu, inst_pc: u32) {
    println!("breakpoint at 0x{0:>08x}", inst_pc);
    loop {
        println!("command >");
        
        let s: String = read();

        if s == "c" {
            break;
        }
    }
}

fn main() {
    let mut f = fs::File::open("./data/sgemm.bin").unwrap();

    let mut buf = vec![];
    f.read_to_end(&mut buf).unwrap();

    let mut insts = vec![0u64; buf.len() / 8];
    LittleEndian::read_u64_into(&buf, insts.as_mut_slice());
    
    let p: usize = 96;
    let q: usize = 363;
    let r: usize = 3072;

    const P_DIV: usize = 2;
    const R_DIV: usize = 6;
    const N_THREADS: usize = P_DIV * R_DIV;

    assert!(p%16 == 0 && p >= P_DIV*16);
    assert!(q >= 2);
    assert!(r%64 == 0 && r >= R_DIV*64);

    let mut a_matrix = vec![0.0f32; p * q];
    let mut b_matrix = vec![0.0f32; q * r];
    let mut c_matrix = vec![0.0f32; p * r];

    let a_stride: usize = q;
    let b_stride: usize = r;
    let c_stride: usize = r;

    let a_addr = 1024 * 4;
    let b_addr = a_addr + a_matrix.len() * 4;
    let c_addr = b_addr + b_matrix.len() * 4;
    
    let mut rng = rand::thread_rng();
    let distrib = Uniform::new(0.0, 1.0);

    for i in 0..a_matrix.len() {
        a_matrix[i] = distrib.sample(&mut rng);
    }

    for i in 0..b_matrix.len() {
        b_matrix[i] = distrib.sample(&mut rng);
    }

    for i in 0..c_matrix.len() {
        c_matrix[i] = distrib.sample(&mut rng);
    }

    let mut emu = QPUEmu::new((1024 + a_matrix.len() + b_matrix.len() + c_matrix.len()) * 4, breakpoint_handler);

    let mut th = 0;
    let h = (p+16*P_DIV-1)/(16*P_DIV);
    let w = (r+64*R_DIV-1)/(64*R_DIV);
    
    let alpha: f32 = 1.0;
    let beta: f32 = 1.0;

    const UNIFORM_SIZE: usize = 14;

    let mut uniforms: [[u32; UNIFORM_SIZE]; N_THREADS] = [[0; UNIFORM_SIZE]; N_THREADS];

    let mut uniform_ptrs = vec![0u32; N_THREADS];
    for th in 0..uniform_ptrs.len() {
        uniform_ptrs[th] = (th * UNIFORM_SIZE * 4) as u32;
    }

    for i in 0..P_DIV {
        for j in 0..R_DIV {
            let p_idx = if i != P_DIV-1 { h as u32 } else { ((p-i*h*16) / 16) as u32 };
            let q_idx = q as u32;
            let r_idx = if j != R_DIV-1 { w as u32 } else { ((r-j*w*64) / 64) as u32 };

            uniforms[th][0] = (th * UNIFORM_SIZE * 4) as u32;
            uniforms[th][1] = p_idx;
            uniforms[th][2] = q_idx;
            uniforms[th][3] = r_idx;
            uniforms[th][4] = (a_addr + (a_stride * (i*16*h) + (0     )) * 4) as u32;
            uniforms[th][5] = (b_addr + (b_stride * (0     ) + (j*64*w)) * 4) as u32;
            uniforms[th][6] = (c_addr + (c_stride * (i*16*h) + (j*64*w)) * 4) as u32;
            th += 1;
        }
    }

    for th in 0..N_THREADS {
        uniforms[th][7] = (a_stride * 4) as u32;
        uniforms[th][8] = (b_stride * 4) as u32;
        uniforms[th][9] = (c_stride * 4) as u32;
        uniforms[th][10] = f32_to_u32(alpha);
        uniforms[th][11] = f32_to_u32(beta);
        uniforms[th][12] = th as u32;
        uniforms[th][13] = N_THREADS as u32;
    }

    for th in 0..N_THREADS {
        for i in 0..UNIFORM_SIZE {
            let bytes = unsafe {
                std::mem::transmute::<u32, [u8; 4]>(uniforms[th][i])
            };
            for b in 0..4 {
                emu.mem[(th * UNIFORM_SIZE + i) * 4 + b] = bytes[b];
            }
        }
    }

    for idx in 0..a_matrix.len() {
        let bytes = unsafe {
            std::mem::transmute::<f32, [u8; 4]>(a_matrix[idx])
        };
        for b in 0..4 {
            emu.mem[a_addr + idx * 4 + b] = bytes[b];
        }
    }

    for idx in 0..b_matrix.len() {
        let bytes = unsafe {
            std::mem::transmute::<f32, [u8; 4]>(b_matrix[idx])
        };
        for b in 0..4 {
            emu.mem[b_addr + idx * 4 + b] = bytes[b];
        }
    }

    for idx in 0..c_matrix.len() {
        let bytes = unsafe {
            std::mem::transmute::<f32, [u8; 4]>(c_matrix[idx])
        };
        for b in 0..4 {
            emu.mem[c_addr + idx * 4 + b] = bytes[b];
        }
    }

    let mut ref_matrix = c_matrix.clone();
    sgemm(p, r, q, alpha, &a_matrix, &b_matrix, beta, &mut ref_matrix);

    let start = Instant::now();

    emu.execute(&insts, &uniform_ptrs, N_THREADS);

    for idx in 0..c_matrix.len() {
        let mut bytes = [0u8; 4];
        for b in 0..4 {
            bytes[b] = emu.mem[c_addr + idx * 4 + b];
        }
        c_matrix[idx] = unsafe {
            std::mem::transmute::<[u8; 4], f32>(bytes)
        };
    }

    assert_eq!(c_matrix, ref_matrix);

    let end = start.elapsed();
    println!("{}.{:03} elapsed.", end.as_secs(), end.subsec_nanos() / 1000000);
}

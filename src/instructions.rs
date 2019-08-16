use crate::utils::*;
use crate::constants::*;
    
#[derive(Debug, Clone)]
pub struct InstFormatAlu {
    pub sig : u8,
    pub unpack : u8,
    pub pm : u8,
    pub pack : u8,
    pub cond_add : u8,
    pub cond_mul : u8,
    pub sf : u8,
    pub ws : u8,
    pub waddr_add : u8,
    pub waddr_mul : u8,
    pub op_mul : u8,
    pub op_add : u8,
    pub raddr_a : u8,
    pub raddr_b : u8,
    pub add_a : u8,
    pub add_b : u8,
    pub mul_a : u8,
    pub mul_b : u8,
}

#[derive(Debug)]
pub struct InstFormatAluSmallImm {
    pub unpack : u8,
    pub pm : u8,
    pub pack : u8,
    pub cond_add : u8,
    pub cond_mul : u8,
    pub sf : u8,
    pub ws : u8,
    pub waddr_add : u8,
    pub waddr_mul : u8,
    pub op_mul : u8,
    pub op_add : u8,
    pub raddr_a : u8,
    pub small_immed : u8,
    pub add_a : u8,
    pub add_b : u8,
    pub mul_a : u8,
    pub mul_b : u8,
}

#[derive(Debug)]
pub struct InstFormatBranch {
    pub cond_br : u8,
    pub rel : u8,
    pub reg : u8,
    pub raddr_a : u8,
    pub ws: u8,
    pub waddr_add : u8,
    pub waddr_mul : u8,
    pub immediate : u32,
}

#[derive(Debug)]
pub struct InstFormatLoadImm32 {
    pub pm : u8,
    pub pack : u8,
    pub cond_add : u8,
    pub cond_mul : u8,
    pub sf : u8,
    pub ws : u8,
    pub waddr_add : u8,
    pub waddr_mul : u8,
    pub immediate : u32,
}

#[derive(Debug)]
pub struct InstFormatLoadImmPerElem {
    pub pm : u8,
    pub pack : u8,
    pub cond_add : u8,
    pub cond_mul : u8,
    pub sf : u8,
    pub ws : u8,
    pub waddr_add : u8,
    pub waddr_mul : u8,
    pub per_element_ms_bit : u16,
    pub per_element_ls_bit : u16,
}

#[derive(Debug)]
pub struct InstFormatSemaphore {
    pub pm : u8,
    pub pack : u8,
    pub cond_add : u8,
    pub cond_mul : u8,
    pub sf : u8,
    pub ws : u8,
    pub waddr_add : u8,
    pub waddr_mul : u8,
    pub sa: u8,
    pub semaphore: u8,
}

#[derive(Debug)]
pub enum InstFormat {
    Alu(InstFormatAlu),
    AluSmallImm(InstFormatAluSmallImm),
    Branch(InstFormatBranch),
    LoadImm32(InstFormatLoadImm32),
    LoadImmPerElemSigned(InstFormatLoadImmPerElem),
    LoadImmPerElemUnsigned(InstFormatLoadImmPerElem),
    Semaphore(InstFormatSemaphore)
}

pub fn decode_inst(inst: u64) -> InstFormat {
    let sig = get_bits(inst, 63, 60) as u8;

    match sig {
        SIG_NOPSI => InstFormat::AluSmallImm(
            InstFormatAluSmallImm {
                unpack      : get_bits(inst, 59, 57) as u8,
                pm          : get_bits(inst, 56, 56) as u8,
                pack        : get_bits(inst, 55, 52) as u8,
                cond_add    : get_bits(inst, 51, 49) as u8,
                cond_mul    : get_bits(inst, 48, 46) as u8,
                sf          : get_bits(inst, 45, 45) as u8,
                ws          : get_bits(inst, 44, 44) as u8,
                waddr_add   : get_bits(inst, 43, 38) as u8,
                waddr_mul   : get_bits(inst, 37, 32) as u8,
                op_mul      : get_bits(inst, 31, 29) as u8,
                op_add      : get_bits(inst, 28, 24) as u8,
                raddr_a     : get_bits(inst, 23, 18) as u8,
                small_immed : get_bits(inst, 17, 12) as u8,
                add_a       : get_bits(inst, 11, 9) as u8,
                add_b       : get_bits(inst, 8, 6) as u8,
                mul_a       : get_bits(inst, 5, 3) as u8,
                mul_b       : get_bits(inst, 2, 0) as u8
            }),
        SIG_BRA => InstFormat::Branch(
            InstFormatBranch {
                cond_br     : get_bits(inst, 55, 52) as u8,
                rel         : get_bits(inst, 51, 51) as u8,
                reg         : get_bits(inst, 50, 50) as u8,
                raddr_a     : get_bits(inst, 49, 45) as u8,
                ws          : get_bits(inst, 44, 44) as u8,
                waddr_add   : get_bits(inst, 43, 38) as u8,
                waddr_mul   : get_bits(inst, 37, 32) as u8,
                immediate   : get_bits(inst, 31, 0) as u32
            }),
        SIG_LDI =>
        {
            let unpack = get_bits(inst, 59, 57) as u8;
            match unpack {
                0b000 => InstFormat::LoadImm32(
                    InstFormatLoadImm32 {
                        pm          : get_bits(inst, 56, 56) as u8,
                        pack        : get_bits(inst, 55, 52) as u8,
                        cond_add    : get_bits(inst, 51, 49) as u8,
                        cond_mul    : get_bits(inst, 48, 46) as u8,
                        sf          : get_bits(inst, 45, 45) as u8,
                        ws          : get_bits(inst, 44, 44) as u8,
                        waddr_add   : get_bits(inst, 43, 38) as u8,
                        waddr_mul   : get_bits(inst, 37, 32) as u8,
                        immediate   : get_bits(inst, 31, 0) as u32
                    }),
                0b001 => InstFormat::LoadImmPerElemSigned(
                    InstFormatLoadImmPerElem {
                        pm          : get_bits(inst, 56, 56) as u8,
                        pack        : get_bits(inst, 55, 52) as u8,
                        cond_add    : get_bits(inst, 51, 49) as u8,
                        cond_mul    : get_bits(inst, 48, 46) as u8,
                        sf          : get_bits(inst, 45, 45) as u8,
                        ws          : get_bits(inst, 44, 44) as u8,
                        waddr_add   : get_bits(inst, 43, 38) as u8,
                        waddr_mul   : get_bits(inst, 37, 32) as u8,
                        per_element_ms_bit   : get_bits(inst, 31, 16) as u16,
                        per_element_ls_bit   : get_bits(inst, 15, 0) as u16
                    }),
                0b011 => InstFormat::LoadImmPerElemUnsigned(
                    InstFormatLoadImmPerElem {
                        pm          : get_bits(inst, 56, 56) as u8,
                        pack        : get_bits(inst, 55, 52) as u8,
                        cond_add    : get_bits(inst, 51, 49) as u8,
                        cond_mul    : get_bits(inst, 48, 46) as u8,
                        sf          : get_bits(inst, 45, 45) as u8,
                        ws          : get_bits(inst, 44, 44) as u8,
                        waddr_add   : get_bits(inst, 43, 38) as u8,
                        waddr_mul   : get_bits(inst, 37, 32) as u8,
                        per_element_ms_bit   : get_bits(inst, 31, 16) as u16,
                        per_element_ls_bit   : get_bits(inst, 15, 0) as u16
                    }),
                0b100 => InstFormat::Semaphore(
                    InstFormatSemaphore {
                        pm          : get_bits(inst, 56, 56) as u8,
                        pack        : get_bits(inst, 55, 52) as u8,
                        cond_add    : get_bits(inst, 51, 49) as u8,
                        cond_mul    : get_bits(inst, 48, 46) as u8,
                        sf          : get_bits(inst, 45, 45) as u8,
                        ws          : get_bits(inst, 44, 44) as u8,
                        waddr_add   : get_bits(inst, 43, 38) as u8,
                        waddr_mul   : get_bits(inst, 37, 32) as u8,
                        sa          : get_bits(inst, 4, 4) as u8,
                        semaphore   : get_bits(inst, 3, 0) as u8
                    }),
                _ => panic!()
            }
        }
        _  => InstFormat::Alu(
            InstFormatAlu{
                sig         : sig,
                unpack      : get_bits(inst, 59, 57) as u8,
                pm          : get_bits(inst, 56, 56) as u8,
                pack        : get_bits(inst, 55, 52) as u8,
                cond_add    : get_bits(inst, 51, 49) as u8,
                cond_mul    : get_bits(inst, 48, 46) as u8,
                sf          : get_bits(inst, 45, 45) as u8,
                ws          : get_bits(inst, 44, 44) as u8,
                waddr_add   : get_bits(inst, 43, 38) as u8,
                waddr_mul   : get_bits(inst, 37, 32) as u8,
                op_mul      : get_bits(inst, 31, 29) as u8,
                op_add      : get_bits(inst, 28, 24) as u8,
                raddr_a     : get_bits(inst, 23, 18) as u8,
                raddr_b     : get_bits(inst, 17, 12) as u8,
                add_a       : get_bits(inst, 11, 9) as u8,
                add_b       : get_bits(inst, 8, 6) as u8,
                mul_a       : get_bits(inst, 5, 3) as u8,
                mul_b       : get_bits(inst, 2, 0) as u8
            })
    }
}
use crate::constants::*;
use super::instructions::*;
use super::utils::*;

pub struct RegisterFile<T: Copy> {
    num_elems: usize,
	regs: Vec<T>
}

impl<T: Copy> RegisterFile<T>  {
    pub fn new(num_elems: usize, count: usize, default: T) -> RegisterFile<T> {
        RegisterFile {
            num_elems: num_elems,
            regs: vec![default; num_elems * count]
        }
    }

    pub fn get(&self, elem: usize, idx: usize) -> T {
        self.regs[self.num_elems * idx + elem]
    }

    pub fn set(&mut self, elem: usize, idx: usize, val: T) {
        self.regs[self.num_elems * idx + elem] = val
    }

    pub fn get_vec(&self, idx: usize) -> &[T] {
        let beg = self.num_elems * idx;
        let end = self.num_elems * (idx + 1);
        &self.regs.as_slice()[beg..end]
    }

    pub fn set_vec(&mut self, idx: usize, vals: &[Option<T>]) {
        for elem in 0..self.num_elems {
            if let Some(val) = vals[elem] {
                self.set(elem, idx, val);
            }
        }
    }
}

pub struct VPMWrite {
    stride: usize,
    horizontal: bool,
    laned: bool,
    size: usize,
    addr: usize,
}

impl VPMWrite {
    pub fn new() -> Self {
        VPMWrite {
            stride: 0,
            horizontal: true,
            laned: false,
            size: 0,
            addr: 0,
        }
    }
}

pub struct VPMDMAStore {
    units: u32,
    depth: u32,
    laned: bool,
    horiz: bool,
    vpmbase: u32,
    modew: u32,
    blockmode: u32,
    stride: u32,
}

impl VPMDMAStore {
    pub fn new() -> Self {
        VPMDMAStore {
            units: 0,
            depth: 0,
            laned: false,
            horiz: true,
            vpmbase: 0,
            modew: 0,
            blockmode: 0,
            stride: 0,
        }
    }
}

pub struct VPMRead {
    num: usize,
    stride: usize,
    horizontal: bool,
    laned: bool,
    size: usize,
    addr: usize,
}

impl VPMRead {
    pub fn new() -> Self {
        VPMRead {
            num: 0,
            stride: 0,
            horizontal: true,
            laned: false,
            size: 0,
            addr: 0,
        }
    }
}

pub struct VPMDMALoad {
    modew: u32,
    mpitch: u32,
    rowlen: usize,
    nrows: usize,
    vpitch: u32,
    vert: bool,
    addrxy: u32,
    mpitchb: u32,
}

impl VPMDMALoad {
    pub fn new() -> Self {
        VPMDMALoad {
            modew: 0,
            mpitch: 0,
            rowlen: 0,
            nrows: 0,
            vpitch: 0,
            vert: false,
            addrxy: 0,
            mpitchb: 0,
        }
    }
}

use std::collections::VecDeque;

pub struct QPUEmu {
	pc: usize,
	reg_r: RegisterFile<u32>,
	reg_ra: RegisterFile<u32>,
	reg_rb: RegisterFile<u32>,
	insts: Vec<u64>,
    zf: [bool; 16],
    nf: [bool; 16],
    cf: [bool; 16],
    uniform_ptr: u32,
    slots: [(u32, u64); 3],
	pub mem: Vec<u8>,
    vpm: Vec<Vec<u8>>,
    vpm_dma_load: VPMDMALoad,
    vpm_read: VPMRead,
    vpm_dma_store: VPMDMAStore,
    vpm_write: VPMWrite,
    tmu0_req_fifo: VecDeque<(u8, [u32; 16])>, // The first element represents parameter type: s, t, r, b := 0, 1, 2, 3.

    breakpoint_handler: fn(&QPUEmu, u32) -> (),
}

impl QPUEmu {
    pub fn new(mem_size: usize, breakpoint_handler: fn(&QPUEmu, u32) -> ()) -> Self {
        let mut vpm = Vec::new();
        for _ in 0..16 {
            vpm.push(vec![0; 64 * 4]);
        }

        QPUEmu {
            pc: 0,
            reg_r: RegisterFile::new(16, 6, 0),
            reg_ra: RegisterFile::new(16, 32, 0),
            reg_rb: RegisterFile::new(16, 32, 0),
            insts: vec![],
            zf: [true; 16],
            nf: [false; 16],
            cf: [false; 16],
            uniform_ptr: 0,
            slots: [(0, 1<<60); 3],
            mem: vec![0; mem_size],
            vpm: vpm,
            vpm_dma_load: VPMDMALoad::new(),
            vpm_read: VPMRead::new(),
            vpm_dma_store: VPMDMAStore::new(),
            vpm_write: VPMWrite::new(),
            tmu0_req_fifo: VecDeque::new(),

            breakpoint_handler: breakpoint_handler,
        }
    }

    fn read_mem_u32(&mut self, addr: usize) -> u32 {
        if addr & 3 != 0 {
            panic!("Not aligned by 4bytes.");
        }
        let b0 = self.mem[addr] as u32;
        let b1 = self.mem[addr + 1] as u32;
        let b2 = self.mem[addr + 2] as u32;
        let b3 = self.mem[addr + 3] as u32;

        b3 << 24 | b2 << 16 | b1 << 8 | b0
    }

    fn read_vpm_mem_u32(&mut self, elem: usize, addr: usize) -> u32 {
        if addr & 3 != 0 {
            panic!("Not aligned by 4bytes");
        }

        let mut bytes = [0u8; 4];
        for i in 0..4 {
            bytes[i] = self.vpm[elem][addr + i];
        }

        u8x4_to_u32(bytes)
    }

    fn read_vpm(&mut self, elem: usize) -> u32 {
        match self.vpm_read.size {
            2 => {
                if self.vpm_read.horizontal {
                    let x = 0;
                    let y = get_bits_u32(self.vpm_read.addr as u32, 31, 0) as usize;

                    if elem == 15 {
                        self.vpm_read.addr += self.vpm_read.stride;
                    }

                    self.read_vpm_mem_u32(x + elem, y * 4)
                } else {
                    let x = get_bits_u32(self.vpm_read.addr as u32, 3, 0) as usize;
                    let y = get_bits_u32(self.vpm_read.addr as u32, 31, 4) as usize;

                    if elem == 15 {
                        self.vpm_read.addr += self.vpm_read.stride;
                    }

                    self.read_vpm_mem_u32(x, (y * 16 + elem) * 4)
                }
            },
            1 => unimplemented!(), // TODO
            0 => unimplemented!(), // TODO
            _ => unimplemented!(), // Reserved.
        }
    }

    fn read_ra(&mut self, elem: usize, addr: u8) -> u32 {
        if addr <= RA_RA31 {
            self.reg_ra.get(elem, addr as usize)
        } else if addr == RA_UNIFORM_READ {
            self.read_mem_u32(self.uniform_ptr as usize)
        } else if addr == RA_ELEMENT_NUMBER {
            elem as u32
        } else if addr == RA_NOP {
            0
        } else if addr == RA_MUTEX_ACQUIRE {
            0
        } else if addr == RA_VPM_READ {
            self.read_vpm(elem)
        } else if addr == RA_VPM_LD_BUSY {
            0
        } else if addr == RA_VPM_LD_WAIT {
            0
        } else {
            panic!("The address is out of range.");
        }
    }

    fn read_rb(&mut self, elem: usize, addr: u8) -> u32 {
        if addr <= RB_RB31 {
            self.reg_rb.get(elem, addr as usize)
        } else if addr == RB_UNIFORM_READ {
            self.read_mem_u32(self.uniform_ptr as usize)
        } else if addr == RB_NOP {
            0
        } else if addr == RB_MUTEX_ACQUIRE {
            0
        } else if addr == RB_VPM_READ {
            self.read_vpm(elem)
        } else if addr == RB_VPM_ST_BUSY {
            0
        } else if addr == RB_VPM_ST_WAIT {
            0
        } else {
            panic!("The address is out of range.");
        }
    }

    fn setup_vpm_load(&mut self, command: u32) -> () {
        if get_bits_u32(command, 31, 28) == 9 {
            self.vpm_dma_load.mpitchb = get_bits_u32(command, 15, 0) as u32; // TODO: Check
        } else if get_bits_u32(command, 31, 31) == 1 {
            self.vpm_dma_load.modew = get_bits_u32(command, 30, 28) as u32;
            self.vpm_dma_load.mpitch =  get_bits_u32(command, 27, 24) as u32;
            self.vpm_dma_load.rowlen = get_bits_u32(command, 23, 20) as usize;
            self.vpm_dma_load.nrows = get_bits_u32(command, 19, 16) as usize;
            self.vpm_dma_load.vpitch = get_bits_u32(command, 15, 12) as u32;
            self.vpm_dma_load.vert = get_bits_u32(command, 11, 11) != 0;
            self.vpm_dma_load.addrxy = get_bits_u32(command, 10, 0) as u32;
            
            if self.vpm_dma_load.rowlen == 0 {
                self.vpm_dma_load.rowlen = 16;
            }
            if self.vpm_dma_load.nrows == 0 {
                self.vpm_dma_load.nrows = 16;
            }
            if self.vpm_dma_load.vpitch == 0 {
                self.vpm_dma_load.vpitch = 16;
            }
        } else { // ID = 0
            self.vpm_read.num = get_bits_u32(command, 23, 20) as usize;
            self.vpm_read.stride = get_bits_u32(command, 17, 12) as usize;
            self.vpm_read.horizontal = get_bits_u32(command, 11, 11) != 0;
            self.vpm_read.laned = get_bits_u32(command, 10, 10) != 0;
            self.vpm_read.size = get_bits_u32(command, 9, 8) as usize;
            self.vpm_read.addr = get_bits_u32(command, 7, 0) as usize;
            
            if self.vpm_read.num == 0 {
                self.vpm_read.num = 16;
            }
            if self.vpm_read.stride == 0 {
                self.vpm_read.stride = 64;
            }
        }
    }

    fn execute_vpm_dma_load(&mut self, addr: u32) -> () {
        let mpitch = if self.vpm_dma_load.mpitch != 0 {
            8 * 2u32.pow(self.vpm_dma_load.mpitch)
        } else {
            self.vpm_dma_load.mpitchb
        } as usize;

        let modew = self.vpm_dma_load.modew;
        if modew == 0 { // 32bit width
            let mut vpm_addr = self.vpm_dma_load.addrxy;
            let row_len = self.vpm_dma_load.rowlen;
            let nrows = self.vpm_dma_load.nrows;
            let vpitch = self.vpm_dma_load.vpitch;

            if self.vpm_dma_load.vert {
                unimplemented!();
            }

            let mut mem_addr_row = addr as usize;

            for _ in 0..nrows {
                let mut mem_addr = mem_addr_row;

                for _ in 0..row_len {
                    let x = get_bits_u32(vpm_addr, 3, 0) as usize;
                    let y = get_bits_u32(vpm_addr, 31, 4) as usize;

                    for byte in 0..4 {
                        self.vpm[x][y * 4 + byte] = self.mem[mem_addr + byte];
                    }

                    vpm_addr += vpitch;
                    mem_addr += 4;
                }

                mem_addr_row += mpitch;
            }
        } else if modew >= 2 && modew <= 3 { // 16bit width
            //let vpm_addr = (self.vpm_dma_load.addrxy << 1) | (modew - 2);
            unimplemented!();
        } else if modew >= 4 && modew <= 7 { // 8bit width
            //let vpm_addr = (self.vpm_dma_load.addrxy << 2) | (modew - 4);
            unimplemented!();
        } else {
            panic!("The mode is out of range.");
        }
    }

    fn setup_vpm_store(&mut self, command: u32) -> () {
        if get_bits_u32(command, 31, 30) == 3 {
            self.vpm_dma_store.blockmode = get_bits_u32(command, 16, 16) as u32;
            self.vpm_dma_store.stride = get_bits_u32(command, 15, 0) as u32; // TODO: Check
        } else if get_bits_u32(command, 31, 30) == 2 {
            self.vpm_dma_store.units = get_bits_u32(command, 29, 23) as u32;
            self.vpm_dma_store.depth = get_bits_u32(command, 22, 16) as u32;
            self.vpm_dma_store.laned = get_bits_u32(command, 15, 15) != 0;
            self.vpm_dma_store.horiz = get_bits_u32(command, 14, 14) != 0;
            self.vpm_dma_store.vpmbase = get_bits_u32(command, 13, 3) as u32;
            self.vpm_dma_store.modew = get_bits_u32(command, 2, 0) as u32;

            if self.vpm_dma_store.units == 0 {
                self.vpm_dma_store.units = 128;
            }
            if self.vpm_dma_store.depth == 0 {
                self.vpm_dma_store.depth = 128;
            }
        } else if get_bits_u32(command, 31, 30) == 0 {
            self.vpm_write.stride = get_bits_u32(command, 17, 12) as usize;
            self.vpm_write.horizontal = get_bits_u32(command, 11, 11) != 0;
            self.vpm_write.laned = get_bits_u32(command, 10, 10) != 0;
            self.vpm_write.size = get_bits_u32(command, 9, 8) as usize;
            self.vpm_write.addr = get_bits_u32(command, 7, 0) as usize;

            if self.vpm_write.stride == 0 {
                self.vpm_write.stride = 64;
            }
        } else {
            panic!("The command ID is out of range.");
        }
    }

    fn execute_vpm_dma_store(&mut self, addr: u32) -> () {
        let mstride = self.vpm_dma_store.stride as usize;
        let modew = self.vpm_dma_store.modew;

        if modew == 0 { // 32bit width
            let mut vpm_addr = self.vpm_dma_store.vpmbase;
            let row_len = self.vpm_dma_store.depth as usize;
            let nrows = self.vpm_dma_store.units as usize;
            let vpitch = 1;

            if self.vpm_dma_store.blockmode == 1 {
                unimplemented!();
            }
            if !self.vpm_dma_store.horiz {
                unimplemented!();
            }

            let mut mem_addr = addr as usize;

            for _ in 0..nrows {
                for _ in 0..row_len {
                    let x = get_bits_u32(vpm_addr, 3, 0) as usize;
                    let y = get_bits_u32(vpm_addr, 31, 4) as usize;

                    for byte in 0..4 {
                        self.mem[mem_addr + byte] = self.vpm[x][y * 4 + byte];
                    }

                    vpm_addr += vpitch;
                    mem_addr += 4;
                }

                mem_addr += mstride;
            }
        } else if modew >= 2 && modew <= 3 { // 16bit width
            unimplemented!();
        } else if modew >= 4 && modew <= 7 { // 8bit width
            unimplemented!();
        } else {
            panic!("The mode is out of range.");
        }
    }

    fn write_vpm_mem_u32(&mut self, elem: usize, addr: usize, val: u32) -> () {
        if addr & 3 != 0 {
            panic!("Not aligned by 4bytes");
        }

        let bytes = u32_to_u8x4(val);

        for i in 0..4 {
            self.vpm[elem][addr + i] = bytes[i];
        }
    }

    fn write_vpm(&mut self, values: &[Option<u32>; 16]) -> () {
        match self.vpm_write.size {
            2 => {
                if self.vpm_write.horizontal {
                    let x = 0;
                    let y = get_bits_u32(self.vpm_write.addr as u32, 5, 0) as usize;

                    self.vpm_write.addr += self.vpm_write.stride;

                    for elem in 0..16 {
                        if let Some(value) = values[elem] {
                            self.write_vpm_mem_u32(x + elem, y * 4, value);
                        }
                    }
                } else {
                    let x = get_bits_u32(self.vpm_write.addr as u32, 3, 0) as usize;
                    let y = get_bits_u32(self.vpm_write.addr as u32, 31, 4) as usize;

                    self.vpm_write.addr += self.vpm_write.stride;

                    for elem in 0..16 {
                        if let Some(value) = values[elem] {
                            self.write_vpm_mem_u32(x, (y * 16 + elem) * 4, value);
                        }
                    }
                }
            },
            1 => unimplemented!(),
            0 => unimplemented!(),
            _ => unimplemented!(), // Reserved.
        }
    }

    fn write_ra(&mut self, addr: u8, values: &[Option<u32>; 16]) -> () {
        if addr >= WA_RA0 && addr <= WA_RA31 {
            self.reg_ra.set_vec(addr as usize, values);
        } else if addr == WA_ACC0 {
            self.reg_r.set_vec(0, values);
        } else if addr == WA_ACC1 {
            self.reg_r.set_vec(1, values);
        } else if addr == WA_ACC2 {
            self.reg_r.set_vec(2, values);
        } else if addr == WA_ACC3 {
            self.reg_r.set_vec(3, values);
        } else if addr == WB_ACC5 {
            unimplemented!();
        } else if addr == WA_NOP {
            // Nop
        } else if addr == WA_UNIFORMS_ADDRESS {
            if let Some(value) = values[0] {
                self.uniform_ptr = value;
            }
        } else if addr == WA_TMU_NOSWAP {
            // TODO: not implemented
        } else if addr == WA_TMU0_S {
            if self.tmu0_req_fifo.len() >= 8 {
                panic!("TMU0 request fifo is overflow.");
            }
            self.tmu0_req_fifo.push_back((0, unwrap_u32x16(values)));
        } else if addr == WA_TMU0_T {
            if self.tmu0_req_fifo.len() >= 8 {
                panic!("TMU0 request fifo is overflow.");
            }
            self.tmu0_req_fifo.push_back((1, unwrap_u32x16(values)));
        } else if addr == WA_TMU0_R {
            if self.tmu0_req_fifo.len() >= 8 {
                panic!("TMU0 request fifo is overflow.");
            }
            self.tmu0_req_fifo.push_back((2, unwrap_u32x16(values)));
        } else if addr == WA_TMU0_B {
            if self.tmu0_req_fifo.len() >= 8 {
                panic!("TMU0 request fifo is overflow.");
            }
            self.tmu0_req_fifo.push_back((3, unwrap_u32x16(values)));
        } else if addr == WA_VPM_WRITE {
            self.write_vpm(values);
        } else if addr == WA_VPMVCD_RD_SETUP {
            if let Some(value) = values[0] {
                self.setup_vpm_load(value);
            }
        } else if addr == WA_VPM_LD_ADDR {
            if let Some(value) = values[0] {
                self.execute_vpm_dma_load(value);
            }
        } else if addr == WA_MUTEX_RELEASE {
            // TODO: not implemented
        } else if addr == WA_HOST_INT {
            // TODO: not implemented
        } else {
            panic!("Invalid address.")
        }
    }

    fn write_rb(&mut self, addr: u8, values: &[Option<u32>; 16]) -> () {
        if addr >= WB_RB0 && addr <= WB_RB31 {
            self.reg_rb.set_vec(addr as usize, values);
        } else if addr == WB_ACC0 {
            self.reg_r.set_vec(0, values);
        } else if addr == WB_ACC1 {
            self.reg_r.set_vec(1, values);
        } else if addr == WB_ACC2 {
            self.reg_r.set_vec(2, values);
        } else if addr == WB_ACC3 {
            self.reg_r.set_vec(3, values);
        } else if addr == WB_ACC5 {
            for elem in 0..16 {
                if let Some(value) = values[0] {
                    self.reg_r.set(elem, 5, value);
                }
            }
        } else if addr == WB_NOP {
            // Nop
        } else if addr == WB_UNIFORMS_ADDRESS {
            if let Some(value) = values[0] {
                self.uniform_ptr = value;
            }
        } else if addr == WB_TMU_NOSWAP {
            // TODO: not implemented
        } else if addr == WB_TMU0_S {
            if self.tmu0_req_fifo.len() >= 8 {
                panic!("TMU0 request fifo is overflow.");
            }
            self.tmu0_req_fifo.push_back((0, unwrap_u32x16(values)));
        } else if addr == WB_TMU0_T {
            if self.tmu0_req_fifo.len() >= 8 {
                panic!("TMU0 request fifo is overflow.");
            }
            self.tmu0_req_fifo.push_back((1, unwrap_u32x16(values)));
        } else if addr == WB_TMU0_R {
            if self.tmu0_req_fifo.len() >= 8 {
                panic!("TMU0 request fifo is overflow.");
            }
            self.tmu0_req_fifo.push_back((2, unwrap_u32x16(values)));
        } else if addr == WB_TMU0_B {
            if self.tmu0_req_fifo.len() >= 8 {
                panic!("TMU0 request fifo is overflow.");
            }
            self.tmu0_req_fifo.push_back((3, unwrap_u32x16(values)));
        } else if addr == WB_VPM_WRITE {
            self.write_vpm(values);
        } else if addr == WB_VPMVCD_WR_SETUP {
            if let Some(value) = values[0] {
                self.setup_vpm_store(value);
            }
        } else if addr == WB_VPM_ST_ADDR {
            if let Some(value) = values[0] {
                self.execute_vpm_dma_store(value);
            }
        } else if addr == WB_MUTEX_RELEASE {
            // TODO: not implemented
        } else if addr == WB_HOST_INT {
            // TODO: not implemented
        } else {
            panic!("Invalid address.")
        }
    }

    fn mux_add_a(&mut self, elem: usize, add_a: u8, val: u32, rb_val: u32) -> u32 {
        match add_a {
            ALU_SRC_R0 => self.reg_r.get(elem, 0),
            ALU_SRC_R1 => self.reg_r.get(elem, 1),
            ALU_SRC_R2 => self.reg_r.get(elem, 2),
            ALU_SRC_R3 => self.reg_r.get(elem, 3),
            ALU_SRC_R4 => self.reg_r.get(elem, 4),
            ALU_SRC_R5 => self.reg_r.get(elem, 5),
            ALU_SRC_RA => val,
            ALU_SRC_RB => rb_val,
            _ => panic!("Invalid source.")
        }
    }

    fn mux_add_b(&mut self, elem: usize, add_b: u8, val: u32, rb_val: u32) -> u32 {
        match add_b {
            ALU_SRC_R0 => self.reg_r.get(elem, 0),
            ALU_SRC_R1 => self.reg_r.get(elem, 1),
            ALU_SRC_R2 => self.reg_r.get(elem, 2),
            ALU_SRC_R3 => self.reg_r.get(elem, 3),
            ALU_SRC_R4 => self.reg_r.get(elem, 4),
            ALU_SRC_R5 => self.reg_r.get(elem, 5),
            ALU_SRC_RA => val,
            ALU_SRC_RB => rb_val,
            _ => panic!("Invalid source.")
        }
    }

    fn perform_add_alu(op: u8, val1: u32, val2: u32) -> u32 {
        let i32_val1 = val1 as i32;
        let i32_val2 = val2 as i32;
        let f32_val1 = u32_to_f32(val1);
        let f32_val2 = u32_to_f32(val2);

        match op {
            ADDOP_NOP => 0,
            ADDOP_FADD => f32_to_u32(f32_val1 + f32_val2),
            ADDOP_FSUB => f32_to_u32(f32_val1 - f32_val2),
            ADDOP_FMIN => f32_to_u32(f32_val1.min(f32_val2)),
            ADDOP_FMAX => f32_to_u32(f32_val1.max(f32_val2)),
            ADDOP_FMINABS => f32_to_u32(f32_val1.abs().min(f32_val2.abs())),
            ADDOP_FMAXABS => f32_to_u32(f32_val1.abs().max(f32_val2.abs())),
            ADDOP_FTOI => (f32_val1 as i32) as u32,
            ADDOP_ITOF => f32_to_u32(i32_val1 as f32),
            ADDOP_ADD => (i32_val1 + i32_val2) as u32,
            ADDOP_SUB => (i32_val1 - i32_val2) as u32,
            ADDOP_SHR => val1 >> val2,
            ADDOP_ASR => (i32_val1 >> val2) as u32,
            ADDOP_ROR => val1.rotate_right(val2),
            ADDOP_SHL => val1.rotate_left(val2),
            ADDOP_MIN => i32_val1.min(i32_val2) as u32,
            ADDOP_MAX => i32_val1.max(i32_val2) as u32,
            ADDOP_AND => val1 & val2,
            ADDOP_OR => val1 | val2,
            ADDOP_XOR => val1 ^ val2,
            ADDOP_NOT => !val1,
            ADDOP_CLZ => val1.leading_zeros(),
            ADDOP_V8ADDS => unimplemented!(),
            ADDOP_V8SUBS => unimplemented!(),
            _ => panic!("Invalid add operation.")
        }
    }

    fn perform_mul_alu(op: u8, val1: u32, val2: u32) -> u32 {
        let i32_val1 = val1 as i32;
        let i32_val2 = val2 as i32;
        let f32_val1 = u32_to_f32(val1);
        let f32_val2 = u32_to_f32(val2);

        const MASK24BIT: i32 = (1 << 24) - 1;

        match op {
            MULOP_NOP => 0,
            MULOP_FMUL => f32_to_u32(f32_val1 * f32_val2),
            MULOP_MUL24 => ((i32_val1 & MASK24BIT) * (i32_val2 & MASK24BIT)) as u32,
            MULOP_V8MULD => unimplemented!(),
            MULOP_V8MIN => {
                let v8_arr1 = u32_to_u8x4(val1);
                let v8_arr2 = u32_to_u8x4(val2);

                let mut v8_min = [0; 4];
                for i in 0..4 {
                    v8_min[i] = std::cmp::min(v8_arr1[i],v8_arr2[i]);
                }

                u8x4_to_u32(v8_min)
            },
            MULOP_V8MAX => {
                let v8_arr1 = u32_to_u8x4(val1);
                let v8_arr2 = u32_to_u8x4(val2);

                let mut v8_max = [0; 4];
                for i in 0..4 {
                    v8_max[i] = std::cmp::max(v8_arr1[i],v8_arr2[i]);
                }

                u8x4_to_u32(v8_max)
            },
            MULOP_V8ADDS => unimplemented!(),
            MULOP_V8SUBS => unimplemented!(),
            _ => panic!("Invalid multiply operation.")
        }
    }

    fn set_flag(&mut self, value: u32, elem: usize) -> () {
        self.zf[elem] = value == 0;
        self.nf[elem] = (value as i32) < 0;
        self.cf[elem] = false;
    }

    fn execute_tmu0_load(&mut self) {
        let mut param_available = [false; 4];
        let mut param_value = [[0u32; 16]; 4];

        while let Some((param_type, param_val)) = self.tmu0_req_fifo.pop_front() {
            if param_available[param_type as usize] {
                panic!("Parameter duplicated.");
            }

            param_available[param_type as usize] = true;
            param_value[param_type as usize] = param_val;

            if param_type == 0 {
                break;
            }
        }

        if param_available[0] && param_available[1] && param_available[2] {
            panic!("Not implemented for Cube texture."); 
        } else if param_available[0] && param_available[1] {
            panic!("Not implemented for 2D texture.");
        } else if param_available[0] {
            let addr = param_value[0];
            for elem in 0..16 {
                let val = self.read_mem_u32(addr[elem] as usize);
                self.reg_r.set(elem, 4, val);
            }
        } else {
            panic!("Parameter s is required.");
        }
    }

    fn execute_alu(&mut self, fields: &InstFormatAlu) {
        let mut add_alu_results = [None; 16];
        let mut mul_alu_results = [None; 16];

        for elem in 0..16 {
            let do_add = match fields.cond_add {
                COND_NEVER => false,
                COND_ALWAYS => true,
                COND_ZS => self.zf[elem],
                COND_ZC => !self.zf[elem],
                COND_NS => self.nf[elem],
                COND_NC => !self.nf[elem],
                COND_CS => self.cf[elem],
                COND_CC => !self.cf[elem],
                _ => panic!("Invalid condition code.")
            } && fields.op_add != ADDOP_NOP;
            
            let do_mul = match fields.cond_mul {
                COND_NEVER => false,
                COND_ALWAYS => true,
                COND_ZS => self.zf[elem],
                COND_ZC => !self.zf[elem],
                COND_NS => self.nf[elem],
                COND_NC => !self.nf[elem],
                COND_CS => self.cf[elem],
                COND_CC => !self.cf[elem],
                _ => panic!("Invalid condition code.")
            } && fields.op_mul != MULOP_NOP;

            let ra_val = self.read_ra(elem, fields.raddr_a);
            let rb_val = self.read_rb(elem, fields.raddr_b);

            if do_add {
                let add_a_val = self.mux_add_a(elem, fields.add_a, ra_val, rb_val);
                let add_b_val = self.mux_add_b(elem, fields.add_b, ra_val, rb_val);

                let add_result = QPUEmu::perform_add_alu(fields.op_add, add_a_val, add_b_val);

                add_alu_results[elem] = Some(add_result);
                
                if fields.sf != 0 {
                    self.set_flag(add_result, elem);
                }
            }
            
            if do_mul {
                let mul_a_val = self.mux_add_a(elem, fields.mul_a, ra_val, rb_val);
                let mul_b_val = self.mux_add_b(elem, fields.mul_b, ra_val, rb_val);

                let mul_result = QPUEmu::perform_mul_alu(fields.op_mul, mul_a_val, mul_b_val);

                mul_alu_results[elem] = Some(mul_result);

                if fields.sf != 0 {
                    if !do_add {
                        self.set_flag(mul_result, elem);
                    }
                }
            }
        }

        if fields.ws == 0 {
            self.write_ra(fields.waddr_add, &add_alu_results);
            self.write_rb(fields.waddr_mul, &mul_alu_results);
        } else {
            self.write_rb(fields.waddr_add, &add_alu_results);
            self.write_ra(fields.waddr_mul, &mul_alu_results);
        }

        if fields.raddr_a == RA_UNIFORM_READ || fields.raddr_b == RB_UNIFORM_READ {
            self.uniform_ptr = self.uniform_ptr + 4;
        }

        if fields.sig == SIG_LDTMU0 {
            self.execute_tmu0_load();
        }
    }

    fn decode_small_imm(&mut self, imm: u8) -> (u32, usize) {
        let imm_val = if imm <= 31 {
            sign_extend(imm as u32, 5)
        } else if imm <= 39 {
            let fval : f32 = (1 << (imm - 32)) as f32;
            unsafe {
                std::mem::transmute::<f32, u32>(fval)
            }
        } else if imm >= 48 && imm <= 63 {
            0
        } else {
            panic!()
        };

        let rotate_val = if imm <= 47 {
            0
        } else if imm == 48 {
            panic!()
        } else if imm >= 49 && imm <= 63 {
            imm as usize - 48
        } else {
            panic!()
        };

        (imm_val, rotate_val)
    }

    fn execute_alu_small_imm(&mut self, fields: &InstFormatAluSmallImm) {
        let mut add_alu_results = [None; 16];
        let mut mul_alu_results = [None; 16];
        
        let (rb_val, rotate) = self.decode_small_imm(fields.small_immed);

        for elem in 0..16 {

            let do_add = match fields.cond_add {
                COND_NEVER => false,
                COND_ALWAYS => true,
                COND_ZS => self.zf[elem],
                COND_ZC => !self.zf[elem],
                COND_NS => self.nf[elem],
                COND_NC => !self.nf[elem],
                COND_CS => self.cf[elem],
                COND_CC => !self.cf[elem],
                _ => panic!()
            } && fields.op_add != ADDOP_NOP;
            
            let do_mul = match fields.cond_mul {
                COND_NEVER => false,
                COND_ALWAYS => true,
                COND_ZS => self.zf[elem],
                COND_ZC => !self.zf[elem],
                COND_NS => self.nf[elem],
                COND_NC => !self.nf[elem],
                COND_CS => self.cf[elem],
                COND_CC => !self.cf[elem],
                _ => panic!()
            } && fields.op_mul != MULOP_NOP;

            let ra_val = self.read_ra(elem, fields.raddr_a);
            let rotated_elem = (elem + rotate) % 16;

            if do_add {
                let add_a_val = self.mux_add_a(elem, fields.add_a, ra_val, rb_val);
                let add_b_val = self.mux_add_b(elem, fields.add_b, ra_val, rb_val);

                let add_result = QPUEmu::perform_add_alu(fields.op_add, add_a_val, add_b_val);

                add_alu_results[rotated_elem] = Some(add_result);
                
                if fields.sf != 0 {
                    self.set_flag(add_result, rotated_elem);
                }
            }
            
            if do_mul {
                let mul_a_val = self.mux_add_a(elem, fields.mul_a, ra_val, rb_val);
                let mul_b_val = self.mux_add_b(elem, fields.mul_b, ra_val, rb_val);

                let mul_result = QPUEmu::perform_mul_alu(fields.op_mul, mul_a_val, mul_b_val);

                mul_alu_results[rotated_elem] = Some(mul_result);

                if fields.sf != 0 {
                    if !do_add {
                        self.set_flag(mul_result, rotated_elem);
                    }
                }
            }
        }

        if fields.ws == 0 {
            self.write_ra(fields.waddr_add, &add_alu_results);
            self.write_rb(fields.waddr_mul, &mul_alu_results);
        } else {
            self.write_rb(fields.waddr_add, &add_alu_results);
            self.write_ra(fields.waddr_mul, &mul_alu_results);
        }

        if fields.raddr_a == RA_UNIFORM_READ {
            self.uniform_ptr = self.uniform_ptr + 4;
        }
    }

    fn execute_branch(&mut self, fields: &InstFormatBranch) -> () {
        let br = match fields.cond_br {
            COND_BR_ALWAYS => true,
            COND_BR_ZS => reduction_and(&self.zf, false),
            COND_BR_ZC => reduction_and(&self.zf, true),
            COND_BR_ANYZS => reduction_or(&self.zf, false),
            COND_BR_ANYZC => reduction_or(&self.zf, true),
            COND_BR_NS => reduction_and(&self.nf, false),
            COND_BR_NC => reduction_and(&self.nf, true),
            COND_BR_ANYNS => reduction_or(&self.nf, false),
            COND_BR_ANYNC => reduction_or(&self.nf, true),
            COND_BR_CS => reduction_and(&self.cf, false),
            COND_BR_CC => reduction_and(&self.cf, true),
            COND_BR_ANYCS => reduction_or(&self.cf, false),
            COND_BR_ANYCC => reduction_or(&self.cf, true),
            _ => panic!()
        };

        if br {
            let br_val = if fields.reg != 0 {
                self.reg_ra.get(0, fields.raddr_a as usize)
            } else {
                fields.immediate
            } as i32;

            self.pc = if fields.rel == 0 {
                br_val / 8 - 1
            } else {
                self.pc as i32 + br_val / 8
            } as usize;
        }

        if fields.raddr_a == RA_UNIFORM_READ {
            self.uniform_ptr = self.uniform_ptr + 4;
        }
    }

    fn execute_load_imm32(&mut self, fields: &InstFormatLoadImm32) {
        let add_result = [Some(fields.immediate); 16];
        let mul_result = [Some(fields.immediate); 16];

        if fields.ws == 0 {
            self.write_ra(fields.waddr_add, &add_result);
        } else {
            self.write_rb(fields.waddr_add, &add_result);
        }
        if fields.ws == 0 {
            self.write_rb(fields.waddr_mul, &mul_result);
        } else {
            self.write_ra(fields.waddr_mul, &mul_result);
        }
    }

    fn decode_imm_per_elem(hi: u16, lo: u16, signed: bool, elem: usize) -> u32 {
        let hi_bit = ((hi >> elem) & 0x1) as u32;
        let lo_bit = ((lo >> elem) & 0x1) as u32;

        if signed {
            let abs_val = lo_bit as i32;
            let sign = (hi_bit as i32) * 2 - 1;
            (sign * abs_val) as u32
        } else {
            hi_bit << 31 | lo_bit
        }
    }

    fn execute_load_imm_per_elem(&mut self, fields: &InstFormatLoadImmPerElem, signed: bool) {
        let mut add_alu_results = [None; 16];
        let mut mul_alu_results = [None; 16];

        for elem in 0..16 {
            
            let do_add = match fields.cond_add {
                COND_NEVER => false,
                COND_ALWAYS => true,
                COND_ZS => self.zf[elem],
                COND_ZC => !self.zf[elem],
                COND_NS => self.nf[elem],
                COND_NC => !self.nf[elem],
                COND_CS => self.cf[elem],
                COND_CC => !self.cf[elem],
                _ => panic!()
            };
            
            let do_mul = match fields.cond_mul {
                COND_NEVER => false,
                COND_ALWAYS => true,
                COND_ZS => self.zf[elem],
                COND_ZC => !self.zf[elem],
                COND_NS => self.nf[elem],
                COND_NC => !self.nf[elem],
                COND_CS => self.cf[elem],
                COND_CC => !self.cf[elem],
                _ => panic!()
            };

            if do_add {
                let add_result = QPUEmu::decode_imm_per_elem(fields.per_element_ms_bit, fields.per_element_ls_bit, signed, elem);

                add_alu_results[elem] = Some(add_result);
                
                if fields.sf != 0 {
                    self.set_flag(add_result, elem);
                }
            }
            
            if do_mul {
                let mul_result = QPUEmu::decode_imm_per_elem(fields.per_element_ms_bit, fields.per_element_ls_bit, signed, elem);

                mul_alu_results[elem] = Some(mul_result);

                if fields.sf != 0 {
                    if !do_add {
                        self.set_flag(mul_result, elem);
                    }
                }
            }
        }

        if fields.ws == 0 {
            self.write_ra(fields.waddr_add, &add_alu_results);
            self.write_rb(fields.waddr_mul, &mul_alu_results);
        } else {
            self.write_rb(fields.waddr_add, &add_alu_results);
            self.write_ra(fields.waddr_mul, &mul_alu_results);
        }
    }

    fn execute_semaphore(&mut self, fields: &InstFormatSemaphore) {
    }

    fn execute_inst(&mut self, inst: &InstFormat) -> () {
        match inst {
            InstFormat::Alu(fields) => {
                self.execute_alu(fields);
            },
            InstFormat::AluSmallImm(fields) => {
                self.execute_alu_small_imm(fields);
            },
            InstFormat::LoadImm32(fields) => {
                self.execute_load_imm32(fields);
            },
            InstFormat::LoadImmPerElemSigned(fields) => {
                self.execute_load_imm_per_elem(fields, true);
            },
            InstFormat::LoadImmPerElemUnsigned(fields) => {
                self.execute_load_imm_per_elem(fields, false);
            },
            InstFormat::Branch(fields) => {
                self.execute_branch(fields);
            },
            InstFormat::Semaphore(fields) => {
                self.execute_semaphore(fields);
            },
        }
    }

    pub fn execute(&mut self, insts: &Vec<u64>, uniform_ptrs: &Vec<u32>, n_threads: usize) -> () {
        self.insts = insts.clone();

        for th in 0..n_threads {
            self.uniform_ptr = uniform_ptrs[th];
            self.pc = 0;

            while self.pc < self.insts.len() {
                let (inst_pc, inst) = self.slots[0];

                self.slots[0] = self.slots[1];
                self.slots[1] = self.slots[2];
                self.slots[2] = (self.pc as u32, self.insts[self.pc]);
                
                let decoded_inst = decode_inst(inst);

                if let InstFormat::Alu(alu_inst) = &decoded_inst {
                    if alu_inst.sig == SIG_BPKT {
                        let handler = self.breakpoint_handler;
                        handler(self, inst_pc);
                    }
                }

                self.execute_inst(&decoded_inst);

                self.pc = self.pc + 1;
            }
        }
    }
}


#[test]
fn test_qpu_perform_add_alu() {
    let result = QPUEmu::perform_add_alu(ADDOP_NOP, 1, 2);
    assert_eq!(result, 0);

    let result = QPUEmu::perform_add_alu(ADDOP_FADD, f32_to_u32(1.0), f32_to_u32(2.0));
    assert_eq!(u32_to_f32(result), 3.0);

    let result = QPUEmu::perform_add_alu(ADDOP_FSUB, f32_to_u32(1.0), f32_to_u32(2.0));
    assert_eq!(u32_to_f32(result), -1.0);

    let result = QPUEmu::perform_add_alu(ADDOP_FMIN, f32_to_u32(-1.0), f32_to_u32(2.0));
    assert_eq!(u32_to_f32(result), -1.0);

    let result = QPUEmu::perform_add_alu(ADDOP_FMAX, f32_to_u32(-3.0), f32_to_u32(-1.0));
    assert_eq!(u32_to_f32(result), -1.0);

    let result = QPUEmu::perform_add_alu(ADDOP_ADD, 1, 2);
    assert_eq!(result, 3);
}
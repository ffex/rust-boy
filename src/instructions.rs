use crate::GbGen;

impl GbGen {
    pub fn ds(&mut self, num_bytes: &str, starter_point: &str) {
        //TODO change the string calculation
        self.main_code
            .push(format!("ds {}, {}", num_bytes, starter_point));
    }
    pub fn cp(&mut self, value: u8) {
        self.main_code.push(format!("cp {}", value));
    }
    pub fn call(&mut self, label: &str) {
        self.main_code.push(format!("call {}", label));
    }
    pub fn dec(&mut self, register: &str) {
        self.main_code.push(format!("dec {}", register));
    }
    pub fn inc(&mut self, register: &str) {
        self.main_code.push(format!("inc {}", register));
    }
    // ld section
    pub fn ld_a(&mut self, value: u8) {
        self.main_code.push(format!("ld a, {}", value));
    }
    pub fn ld_b(&mut self, value: u8) {
        self.main_code.push(format!("ld b, {}", value));
    }
    pub fn ld_c(&mut self, value: u8) {
        self.main_code.push(format!("ld c, {}", value));
    }
    pub fn ld_d(&mut self, value: u8) {
        self.main_code.push(format!("ld d, {}", value));
    }
    pub fn ld_e(&mut self, value: u8) {
        self.main_code.push(format!("ld e, {}", value));
    }
    pub fn ld_h(&mut self, value: u8) {
        self.main_code.push(format!("ld h, {}", value));
    }
    pub fn ld_l(&mut self, value: u8) {
        self.main_code.push(format!("ld l, {}", value));
    }
    pub fn ld_bc(&mut self, value: u16) {
        self.main_code.push(format!("ld bc, {}", value));
    }
    pub fn ld_de(&mut self, value: u16) {
        self.main_code.push(format!("ld de, {}", value));
    }
    pub fn ld_hl(&mut self, value: u16) {
        self.main_code.push(format!("ld hl, {}", value));
    }
    pub fn ld_addr_str_a(&mut self, address: &str) {
        self.main_code.push(format!("ld {}, a", address));
    }
    //special lds
    pub fn ld_a_str(&mut self, value: &str) {
        self.main_code.push(format!("ld a, {}", value));
    }
    pub fn ld_bc_str(&mut self, value: &str) {
        self.main_code.push(format!("ld bc, {}", value));
    }
    pub fn ld_de_str(&mut self, value: &str) {
        self.main_code.push(format!("ld de, {}", value));
    }
    pub fn ld_hl_str(&mut self, value: &str) {
        self.main_code.push(format!("ld hl, {}", value));
    }
    pub fn ld_hl_i_str(&mut self, value: &str) {
        self.main_code.push(format!("ld [hli], {}", value));
    }
    pub fn ld_b_str(&mut self, value: &str) {
        self.main_code.push(format!("ld b, {}", value));
    }
    pub fn ld_c_str(&mut self, value: &str) {
        self.main_code.push(format!("ld c, {}", value));
    }
    pub fn ld_d_str(&mut self, value: &str) {
        self.main_code.push(format!("ld d, {}", value));
    }
    pub fn ld_e_str(&mut self, value: &str) {
        self.main_code.push(format!("ld e, {}", value));
    }
    pub fn ld_h_str(&mut self, value: &str) {
        self.main_code.push(format!("ld h, {}", value));
    }
    pub fn ld_l_str(&mut self, value: &str) {
        self.main_code.push(format!("ld l, {}", value));
    }
    // jump sections
    pub fn jp(&mut self, label: &str) {
        self.main_code.push(format!("jp {}", label));
    }
    pub fn jp_cond(&mut self, condition: &str, label: &str) {
        self.main_code.push(format!("jp {}, {}", condition, label));
    }
    pub fn jr(&mut self, condition: &str, label: &str) {
        self.main_code.push(format!("jr {}, {}", condition, label));
    }
    pub fn jr_uncond(&mut self, label: &str) {
        self.main_code.push(format!("jr {}", label));
    }
    pub fn ret(&mut self) {
        self.main_code.push(format!("ret"));
    }
    pub fn ret_cond(&mut self, condition: &str) {
        self.main_code.push(format!("ret {}", condition));
    }
    pub fn add(&mut self, reg_a: &str, reg_b: &str) {
        self.main_code.push(format!("add {}, {}", reg_a, reg_b));
    }
    pub fn adc(&mut self, reg: &str) {
        self.main_code.push(format!("adc {}", reg));
    }
    pub fn sub(&mut self, reg_a: &str, reg_b: &str) {
        self.main_code.push(format!("sub {}, {}", reg_a, reg_b));
    }
    pub fn and(&mut self, value: &str) {
        self.main_code.push(format!("and a, {}", value));
    }
    pub fn or(&mut self, reg_a: &str, reg_b: &str) {
        self.main_code.push(format!("or {}, {}", reg_a, reg_b));
    }
    pub fn xor(&mut self, reg_a: &str, reg_b: &str) {
        self.main_code.push(format!("xor {}, {}", reg_a, reg_b));
    }
    pub fn cp_str(&mut self, value: &str) {
        self.main_code.push(format!("cp a, {}", value));
    }
    pub fn srl(&mut self, register: &str) {
        self.main_code.push(format!("srl {}", register));
    }
    pub fn swap(&mut self, register: &str) {
        self.main_code.push(format!("swap {}", register));
    }
    pub fn daa(&mut self) {
        self.main_code.push(format!("daa"));
    }
    pub fn ldh(&mut self, dest: &str, src: &str) {
        self.main_code.push(format!("ldh {}, {}", dest, src));
    }
}

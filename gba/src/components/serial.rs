// struct just to toss serial registers into, zero intention to implemnt further than this unless im forced to
pub struct Serial {
    pub sio_data: [u16; 4],
    pub siomlt_send: u16,
    pub rcnt: u16,
    pub siocnt: u16,
    pub joycnt: u16,
    pub joy_recv_l: u16,
    pub joy_recv_h: u16,
    pub joy_trans_l: u16,
    pub joy_trans_h: u16,
    pub joystat: u16,
}

impl Serial {
    pub fn new() -> Self {
        Self {
            sio_data: [0xFFFF; 4],
            siomlt_send: 0,
            rcnt: 0,
            siocnt: 0,
            joycnt: 0,
            joy_recv_l: 0,
            joy_recv_h: 0,
            joy_trans_l: 0,
            joy_trans_h: 0,
            joystat: 0,
        }
    }

    pub fn reset_sio_registers(&mut self) {
        self.sio_data = [0xFFFF; 4];
    }
}

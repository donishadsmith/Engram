/*
    https://www.zilog.com/docs/z80/um0080.pdf
    https://www.scilab.org/signal-edge-detection
    Based on the z80 manual, which is one of the cpu's the gb cpu is
    inspired from. First machine cycle can occur with an interrupt
    acknowledgment or the fetch operation of the subsequent instuction.
    While gb is fixes, each machine cycle of the z80 could have variable
    clock cycles (3-6) that could be extended using wait operations.
    First machine cycle could be fetch (3-6 ticks), second machine cycle
    could be a read operation (3 ticks), and third machine cycle would be a
    write operation (3 ticks). CPU sampling occured on the rising edge of
    the clock (0 -> 1) to ensure signal stabilization + synchronisation with
    other pins.

    https://izik1.github.io/gbops/
    https://github.com/sysprog21/jitboy/blob/master/README.md
    1 machine cycle of the gb = 4 clock cycles
    Each operation cycle will:

    - Fetch the next opcode
    - Decode
    - Fetch extra data or literals if needed
    - Record the m-cycles consumes to block/adjust timing later
    - Execute
*/

// https://github.com/mvdnes/rboy/blob/main/src/cpu.rs
// conditional jr/jp/call/ret hold their TAKEN cost; subtract 1/1/3/3 when not taken
// https://gist.github.com/bberak/ca001281bb8431d2706afd31401e802b

// M-cycles; https://github.com/krocki/gb/blob/master/gameboy.c table divided by 4
#[rustfmt::skip]
pub const UNPREFIX_CYCLES: [u8; 256] = [
// +0 +1 +2 +3 +4 +5 +6 +7 +8 +9 +A +B +C +D +E +F
    1, 3, 2, 2, 1, 1, 2, 1, 5, 2, 2, 2, 1, 1, 2, 1, // 00+
    1, 3, 2, 2, 1, 1, 2, 1, 3, 2, 2, 2, 1, 1, 2, 1, // 10+
    3, 3, 2, 2, 1, 1, 2, 1, 3, 2, 2, 2, 1, 1, 2, 1, // 20+
    3, 3, 2, 2, 3, 3, 3, 1, 3, 2, 2, 2, 1, 1, 2, 1, // 30+
    1, 1, 1, 1, 1, 1, 2, 1, 1, 1, 1, 1, 1, 1, 2, 1, // 40+
    1, 1, 1, 1, 1, 1, 2, 1, 1, 1, 1, 1, 1, 1, 2, 1, // 50+
    1, 1, 1, 1, 1, 1, 2, 1, 1, 1, 1, 1, 1, 1, 2, 1, // 60+
    2, 2, 2, 2, 2, 2, 1, 2, 1, 1, 1, 1, 1, 1, 2, 1, // 70+
    1, 1, 1, 1, 1, 1, 2, 1, 1, 1, 1, 1, 1, 1, 2, 1, // 80+
    1, 1, 1, 1, 1, 1, 2, 1, 1, 1, 1, 1, 1, 1, 2, 1, // 90+
    1, 1, 1, 1, 1, 1, 2, 1, 1, 1, 1, 1, 1, 1, 2, 1, // A0+
    1, 1, 1, 1, 1, 1, 2, 1, 1, 1, 1, 1, 1, 1, 2, 1, // B0+
    5, 3, 4, 4, 6, 4, 2, 4, 5, 4, 4, 1, 6, 6, 2, 4, // C0+
    5, 3, 4, 0, 6, 4, 2, 4, 5, 4, 4, 0, 6, 0, 2, 4, // D0+
    3, 3, 2, 0, 0, 4, 2, 4, 4, 1, 4, 0, 0, 0, 2, 4, // E0+
    3, 3, 2, 1, 0, 4, 2, 4, 3, 2, 4, 1, 0, 0, 2, 4, // F0+
];

// Blarggs
#[rustfmt::skip]
pub const PREFIX_CYCLES: [u8; 256] = [
// +0 +1 +2 +3 +4 +5 +6 +7 +8 +9 +A +B +C +D +E +F
    2, 2, 2, 2, 2, 2, 4, 2, 2, 2, 2, 2, 2, 2, 4, 2, // 00+
    2, 2, 2, 2, 2, 2, 4, 2, 2, 2, 2, 2, 2, 2, 4, 2, // 10+
    2, 2, 2, 2, 2, 2, 4, 2, 2, 2, 2, 2, 2, 2, 4, 2, // 20+
    2, 2, 2, 2, 2, 2, 4, 2, 2, 2, 2, 2, 2, 2, 4, 2, // 30+
    2, 2, 2, 2, 2, 2, 3, 2, 2, 2, 2, 2, 2, 2, 3, 2, // 40+
    2, 2, 2, 2, 2, 2, 3, 2, 2, 2, 2, 2, 2, 2, 3, 2, // 50+
    2, 2, 2, 2, 2, 2, 3, 2, 2, 2, 2, 2, 2, 2, 3, 2, // 60+
    2, 2, 2, 2, 2, 2, 3, 2, 2, 2, 2, 2, 2, 2, 3, 2, // 70+
    2, 2, 2, 2, 2, 2, 4, 2, 2, 2, 2, 2, 2, 2, 4, 2, // 80+
    2, 2, 2, 2, 2, 2, 4, 2, 2, 2, 2, 2, 2, 2, 4, 2, // 90+
    2, 2, 2, 2, 2, 2, 4, 2, 2, 2, 2, 2, 2, 2, 4, 2, // A0+
    2, 2, 2, 2, 2, 2, 4, 2, 2, 2, 2, 2, 2, 2, 4, 2, // B0+
    2, 2, 2, 2, 2, 2, 4, 2, 2, 2, 2, 2, 2, 2, 4, 2, // C0+
    2, 2, 2, 2, 2, 2, 4, 2, 2, 2, 2, 2, 2, 2, 4, 2, // D0+
    2, 2, 2, 2, 2, 2, 4, 2, 2, 2, 2, 2, 2, 2, 4, 2, // E0+
    2, 2, 2, 2, 2, 2, 4, 2, 2, 2, 2, 2, 2, 2, 4, 2, // F0+
];

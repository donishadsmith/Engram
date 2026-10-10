.text
.global _start

_start:
    mrs r1, cpsr
    bic r1, r1, #0x1F
    orr r1, r1, #0x12 @ Irq mode

    msr cpsr_c, r1
    mrs r2, cpsr
    cmp r1, r2
    bne fail
    mov r0, #42
    b exit

fail:
    mov r0, #13

exit:
    swi 0xFF0000

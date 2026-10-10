.text
.global _start

_start:
    mrs r1, cpsr
    tst r1, #0x20
    bne fail_check_1

    mov r4, #0
    adr r2, thumb_mode
    orr r2, r2, #1
    bx r2

    .thumb
thumb_mode:
    mov r4, #99
    adr r2, arm_mode
    bx r2

    .align 2
    .arm
arm_mode:
    cmp r4, #99
    bne fail_check_2

    mrs r1, cpsr
    tst r1, #0x20
    bne fail_check_3

    mov r0, #42
    b exit

fail_check_1:
    mov r0, #13
    b exit

fail_check_2:
    mov r0, #14
    b exit

fail_check_3:
    mov r0, #15

exit:
    swi 0xFF0000

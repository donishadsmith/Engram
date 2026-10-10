.text
.global _start

_start:
    mov r5, #0x06000000
    ldr r7, =0x11223344

    str r7, [r5, #5]
    ldr r6, [r5, #4]
    cmp r6, r7
    bne fail_check_1

    ldr r6, [r5, #5]
    cmp r6, r7, ROR#8
    bne fail_check_2

    ldr r6, [r5, #6]
    cmp r6, r7, ROR#16
    bne fail_check_3

    strb r7, [r5, #0x20] @ duplicated byte
    ldrb r11, [r5, #0x21]
    cmp r11, #0x44
    bne fail_check_4

    str r7, [r5, #0x30]
    ldrb r11, [r5, #0x31]
    cmp r11, #0x33
    bne fail_check_5

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
    b exit

fail_check_4:
    mov r0, #16
    b exit

fail_check_5:
    mov r0, #17
    b exit

exit:
    swi 0xFF0000

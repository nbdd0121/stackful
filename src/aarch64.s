# Save all non-volatile registers on stack and return.
fiber_save_raw:
    sub sp, sp, 0xA0
    stp d8, d9, [sp, 0x60]
    stp d10, d11, [sp, 0x70]
    stp d12, d13, [sp, 0x80]
    stp d14, d15, [sp, 0x90]
    # x9 is the saved lr
    stp x9, x19, [sp, 0x00]
    stp x20, x21, [sp, 0x10]
    stp x22, x23, [sp, 0x20]
    stp x24, x25, [sp, 0x30]
    stp x26, x27, [sp, 0x40]
    stp x28, x29, [sp, 0x50]
    ret

# Restore all non-volatile registers and return
fiber_restore_ret_raw:
    ldp d8, d9, [sp, 0x70]
    ldp d10, d11, [sp, 0x80]
    ldp d12, d13, [sp, 0x90]
    ldp d14, d15, [sp, 0xA0]
    ldp lr, x19, [sp, 0x00]
    ldp x20, x21, [sp, 0x10]
    ldp x22, x23, [sp, 0x20]
    ldp x24, x25, [sp, 0x30]
    ldp x26, x27, [sp, 0x40]
    ldp x28, x29, [sp, 0x50]
    add sp, sp, 0xA0
    ret

# fiber_enter: fn(usize, fn(usize) -> usize)
# Enter a fresh stack and call the supplied function
.macro FIBER_ENTER_IMPL
    mov x9, lr
    bl  fiber_save_raw
    # Switch stack and enter
    mov x9, sp
    mov sp, x0
    mov x0, x9
    blr x1
    # Switch stack back and exit
    mov sp, x0
    mov x1, 1
    b   fiber_restore_ret_raw
.endm

# fiber_switch: fn(usize) -> usize
.macro FIBER_SWITCH_IMPL
    mov x9, lr
    bl  fiber_save_raw
    # Switch stack
    mov x9, sp
    mov sp, x0
    mov x0, x9
    mov x1, 0
    b   fiber_restore_ret_raw
.endm

.global fiber_enter
fiber_enter:
    FIBER_ENTER_IMPL

.global _fiber_enter
_fiber_enter:
    FIBER_ENTER_IMPL

.global fiber_switch
fiber_switch:
    FIBER_SWITCH_IMPL

.global _fiber_switch
_fiber_switch:
    FIBER_SWITCH_IMPL

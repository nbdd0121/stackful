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
    ldp x30, x19, [sp, 0x00]
    ldp x20, x21, [sp, 0x10]
    ldp x22, x23, [sp, 0x20]
    ldp x24, x25, [sp, 0x30]
    ldp x26, x27, [sp, 0x40]
    ldp x28, x29, [sp, 0x50]
    add sp, sp, 0xA0
    ret

# fiber_enter: fn(StackPointer, usize, fn(StackPointer, usize) -> !) -> SwitchResult
# Enter a fresh stack and call the supplied function
#ifndef __apple_build_version__
.global fiber_enter
.type fiber_enter, @function
fiber_enter:
#else
.global _fiber_enter
_fiber_enter:
#endif
.cfi_startproc
    # Top of the fresh stack, we use these to store the last function that
    # calls fiber_enter/fiber_switch_enter so that the stack trace can continue
    # past this function.
    sub x0, x0, 0x10
    mov x9, sp
    stp x9, x30, [x0]

    mov x9, x30
    bl  fiber_save_raw

    # Switch stack and enter
    mov x9, x0
    mov x0, sp
    mov sp, x9

    # CFI metadata to instruct unwinder to find our saved info at the top of stack.
    .cfi_def_cfa sp, 16
    .cfi_offset sp, -16
    .cfi_offset x30, -8

    # Save the top-of-stack address in old stack frame; otherwise this will be lost after a switch
    str x9, [x0, -8]

    blr x2
    brk 1
#ifndef __apple_build_version__
.size fiber_enter, .-fiber_enter
#endif
.cfi_endproc

# fiber_switch_enter: fn(StackPointer, usize) -> SwitchResult
.global fiber_switch_enter
.global _fiber_switch_enter
fiber_switch_enter:
_fiber_switch_enter:
    # Extract the saved top-of-stack address
    ldr x3, [x0, -8]

    # Fill the address with new caller info for a proper stack trace
    mov x9, sp
    stp x9, x30, [x3]

    mov x9, x30
    bl  fiber_save_raw

    # Switch stack
    mov x9, x0
    mov x0, sp
    mov sp, x9

    # Save the top-of-stack address in old stack frame again.
    str x3, [x0, -8]

    b   fiber_restore_ret_raw

# fiber_switch_leave: fn(StackPointer, usize) -> SwitchResult
.global fiber_switch_leave
.global _fiber_switch_leave
fiber_switch_leave:
_fiber_switch_leave:
    mov x9, x30
    bl  fiber_save_raw

    # Extract the saved top-of-stack address
    ldr x3, [x0, -8]

    # Switch stack
    mov x9, x0
    mov x0, sp
    mov sp, x9

    # Save the top-of-stack address
    str x3, [x0, -8]

    b   fiber_restore_ret_raw

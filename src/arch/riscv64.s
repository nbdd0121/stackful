# Save all non-volatile registers on stack and return.
fiber_save_raw:
    add sp, sp, -0xD0
    sd ra, 0x00(sp)
    sd s0, 0x08(sp)
    sd s1, 0x10(sp)
    sd s2, 0x18(sp)
    sd s3, 0x20(sp)
    sd s4, 0x28(sp)
    sd s5, 0x30(sp)
    sd s6, 0x38(sp)
    sd s7, 0x40(sp)
    sd s8, 0x48(sp)
    sd s9, 0x50(sp)
    sd s10, 0x58(sp)
    sd s11, 0x60(sp)
    fsd fs0, 0x68(sp)
    fsd fs1, 0x70(sp)
    fsd fs2, 0x78(sp)
    fsd fs3, 0x80(sp)
    fsd fs4, 0x88(sp)
    fsd fs5, 0x90(sp)
    fsd fs6, 0x98(sp)
    fsd fs7, 0xA0(sp)
    fsd fs8, 0xA8(sp)
    fsd fs9, 0xB0(sp)
    fsd fs10, 0xB8(sp)
    fsd fs11, 0xC0(sp)
    jr t0

# Restore all non-volatile registers and return
fiber_restore_ret_raw:
    ld ra, 0x00(sp)
    ld s0, 0x08(sp)
    ld s1, 0x10(sp)
    ld s2, 0x18(sp)
    ld s3, 0x20(sp)
    ld s4, 0x28(sp)
    ld s5, 0x30(sp)
    ld s6, 0x38(sp)
    ld s7, 0x40(sp)
    ld s8, 0x48(sp)
    ld s9, 0x50(sp)
    ld s10, 0x58(sp)
    ld s11, 0x60(sp)
    fld fs0, 0x68(sp)
    fld fs1, 0x70(sp)
    fld fs2, 0x78(sp)
    fld fs3, 0x80(sp)
    fld fs4, 0x88(sp)
    fld fs5, 0x90(sp)
    fld fs6, 0x98(sp)
    fld fs7, 0xA0(sp)
    fld fs8, 0xA8(sp)
    fld fs9, 0xB0(sp)
    fld fs10, 0xB8(sp)
    fld fs11, 0xC0(sp)
    add sp, sp, 0xD0
    ret

# fiber_enter: fn(StackPointer, usize, fn(StackPointer, usize) -> !) -> SwitchResult
# Enter a fresh stack and call the supplied function
.global fiber_enter
.type fiber_enter, @function
fiber_enter:
.cfi_startproc
    # Top of the fresh stack, we use these to store the last function that
    # calls fiber_enter/fiber_switch_enter so that the stack trace can continue
    # past this function.
    add a0, a0, -0x10
    sd sp, 0(a0)
    sd ra, 8(a0)

    jal t0, fiber_save_raw

    # Switch stack and enter
    mv t0, sp
    mv sp, a0
    mv a0, t0

    # CFI metadata to instruct unwinder to find our saved info at the top of stack.
    .cfi_def_cfa sp, 16
    .cfi_offset sp, -16
    .cfi_offset ra, -8

    # Save the top-of-stack address in old stack frame; otherwise this will be lost after a switch
    sd sp, -8(a0)

    jalr a2
    ebreak
.size fiber_enter, .-fiber_enter
.cfi_endproc

# fiber_switch_enter: fn(StackPointer, usize) -> SwitchResult
.global fiber_switch_enter
fiber_switch_enter:
    # Extract the saved top-of-stack address
    ld t1, -8(a0)

    # Fill the address with new caller info for a proper stack trace
    sd sp, 0(t1)
    sd ra, 8(t1)

    jal t0, fiber_save_raw

    # Switch stack
    mv t0, sp
    mv sp, a0
    mv a0, t0

    # Save the top-of-stack address
    sd t1, -8(a0)

    j fiber_restore_ret_raw

# fiber_switch_leave: fn(StackPointer, usize) -> SwitchResult
.global fiber_switch_leave
fiber_switch_leave:
    jal t0, fiber_save_raw

    # Extract the saved top-of-stack address
    ld t1, -8(a0)

    # Switch stack
    mv t0, sp
    mv sp, a0
    mv a0, t0

    # Save the top-of-stack address
    sd t1, -8(a0)

    j fiber_restore_ret_raw

.intel_syntax noprefix

# Save all non-volatile registers on stack and return.
fiber_save_raw:
    pop rax
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    sub rsp, 8
    stmxcsr [rsp]
    fnstcw  [rsp + 4]
    push rax
    ret

# Restore all non-volatile registers and return
fiber_restore_ret_raw:
    fldcw [rsp + 4]
    ldmxcsr [rsp]
    add rsp, 8
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    ret

# fiber_enter: fn(StackPointer, usize, fn(StackPointer, usize) -> !) -> SwitchResult
# Enter a fresh stack and call the supplied function
.global fiber_enter
.global _fiber_enter
.type fiber_enter, @function
fiber_enter:
_fiber_enter:
.cfi_startproc
    # Top of the fresh stack, we use these to store the last function that
    # calls fiber_enter/fiber_switch_enter so that the stack trace can continue
    # past this function.
    sub rdi, 16
    mov rax, [rsp]
    mov [rdi + 8], rax
    mov rax, rsp
    add rax, 8
    mov [rdi], rax

    call fiber_save_raw

    # Switch stack
    xchg rsp, rdi

    # CFI metadata to instruct unwinder to find our saved info at the top of stack.
    .cfi_def_cfa rsp, 16
    .cfi_offset rsp, -16
    .cfi_offset rip, -8

    # Save the top-of-stack address in old stack frame; otherwise this will be lost after a switch
    mov [rdi - 8], rsp

    call rdx
    ud2
.size fiber_enter, .-fiber_enter
.cfi_endproc

# fiber_switch_enter: fn(StackPointer, usize) -> SwitchResult
.global fiber_switch_enter
.global _fiber_switch_enter
fiber_switch_enter:
_fiber_switch_enter:
    # Extract the saved top-of-stack address
    mov rcx, [rdi - 8]

    # Fill the address with new caller info for a proper stack trace
    mov rax, [rsp]
    mov [rcx + 8], rax
    mov rax, rsp
    add rax, 8
    mov [rcx], rax

    call fiber_save_raw

    # Switch stack
    mov rax, rsp
    mov rsp, rdi
    mov rdx, rsi

    # Save the top-of-stack address in old stack frame again.
    mov [rax - 8], rcx

    jmp fiber_restore_ret_raw

# fiber_switch_leave: fn(StackPointer, usize) -> SwitchResult
.global fiber_switch_leave
.global _fiber_switch_leave
fiber_switch_leave:
_fiber_switch_leave:
    call fiber_save_raw

    # Extract the saved top-of-stack address
    mov rcx, [rdi - 8]

    # Switch stack
    mov rax, rsp
    mov rsp, rdi
    mov rdx, rsi

    # Save the top-of-stack address
    mov [rax - 8], rcx

    jmp fiber_restore_ret_raw

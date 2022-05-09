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
    call fiber_save_raw
    # Switch stack and enter
    xchg rsp, rdi
    call rdx

    ud2
.size fiber_enter, .-fiber_enter

# fiber_switch: fn(StackPointer, usize) -> SwitchResult
.global fiber_switch
.global _fiber_switch
fiber_switch:
_fiber_switch:
    call fiber_save_raw
    # Switch stack
    mov rax, rsp
    mov rsp, rdi
    mov rdx, rsi
    jmp fiber_restore_ret_raw

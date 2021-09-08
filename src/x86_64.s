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

# fiber_enter: fn(usize, fn(usize) -> usize)
# Enter a fresh stack and call the supplied function
.macro FIBER_ENTER_IMPL
    call fiber_save_raw
    # Switch stack and enter
    xchg rsp, rdi
    call rsi
    # Switch stack back and exit
    mov rsp, rax
    mov rdx, 1
    jmp fiber_restore_ret_raw
.endm

# fiber_switch: fn(usize) -> usize
.macro FIBER_SWITCH_IMPL
    call fiber_save_raw
    # Switch stack
    mov rax, rsp
    mov rsp, rdi
    mov rdx, 0
    jmp fiber_restore_ret_raw
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

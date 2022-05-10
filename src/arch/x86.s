.intel_syntax noprefix

# Save all non-volatile registers on stack and return.
fiber_save_raw:
    pop eax
    push ebx
    push ebp
    push esi
    push edi
    sub esp, 8
    stmxcsr [esp]
    fnstcw  [esp + 4]
    # For easier reference to arguments from the caller.
    lea ebp, [esp + 24]
    # Load the first argument to EDX
    mov edx, [ebp + 8]
    push eax
    ret

# Restore all non-volatile registers and return
fiber_restore_ret_raw:
    fldcw [esp + 4]
    ldmxcsr [esp]
    add esp, 8
    pop edi
    pop esi
    pop ebp
    pop ebx
    # x86 has this annoying pass on stack convention, and when return value
    # is aggregate, the pointer to the return value is passed to the callee
    # on stack, making it a double-indirection.
    #
    # We let this subroutine absorb the complexity so the code below can
    # return using EAX:EDX.
    mov ecx, [esp + 4]
    mov [ecx], eax
    mov [ecx + 4], edx
    ret 4

# fiber_enter: fn(StackPointer, usize, fn(StackPointer, usize) -> !) -> SwitchResult
# Enter a fresh stack and call the supplied function
.global fiber_enter
.global _fiber_enter
.type fiber_enter, @function
fiber_enter:
_fiber_enter:
.cfi_startproc
    call fiber_save_raw

    # Top of the fresh stack, we use these to store the last function that
    # calls fiber_enter/fiber_switch_enter so that the stack trace can continue
    # past this function.
    sub edx, 8
    mov eax, [ebp]
    mov [edx + 4], eax
    lea eax, [ebp + 4]
    mov [edx], eax

    # Switch stack
    mov eax, esp
    mov esp, edx

    # CFI metadata to instruct unwinder to find our saved info at the top of stack.
    .cfi_def_cfa esp, 16
    .cfi_offset esp, -8
    .cfi_offset eip, -4

    # Save the top-of-stack address in old stack frame; otherwise this will be lost after a switch
    mov [eax - 4], esp

    push [ebp + 12]
    push eax
    call dword ptr [ebp + 16]
    ud2
.size fiber_enter, .-fiber_enter
.cfi_endproc

# fiber_switch_enter: fn(StackPointer, usize) -> SwitchResult
.global fiber_switch_enter
.global _fiber_switch_enter
fiber_switch_enter:
_fiber_switch_enter:
    call fiber_save_raw

    # Extract the saved top-of-stack address
    mov ecx, [edx - 4]

    # Fill the address with new caller info for a proper stack trace
    mov eax, [ebp]
    mov [ecx + 4], eax
    lea eax, [ebp + 4]
    mov [ecx], eax

    # Switch stack
    mov eax, esp
    mov esp, edx
    mov edx, [ebp + 12]

    # Save the top-of-stack address in old stack frame again.
    mov [eax - 4], ecx

    jmp fiber_restore_ret_raw

# fiber_switch_leave: fn(StackPointer, usize) -> SwitchResult
.global fiber_switch_leave
.global _fiber_switch_leave
fiber_switch_leave:
_fiber_switch_leave:
    call fiber_save_raw

    # Extract the saved top-of-stack address
    mov ecx, [edx - 4]

    # Switch stack
    mov eax, esp
    mov esp, edx
    mov edx, [ebp + 12]

    # Save the top-of-stack address
    mov [eax - 4], ecx

    jmp fiber_restore_ret_raw

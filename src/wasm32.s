.section .bss.unwinding_stack,"",@
.type unwinding_stack,@object
# Used to carry the updated `StackPointer` from an entering fiber switch to a leaving fiber switch.
rewinding_stack:
    .skip 4
.size rewinding_stack, 4

# Used to carry the target `StackPointer` from a leaving fiber switch to an entering fiber switch.
unwinding_stack:
    .skip 4
.size unwinding_stack, 4

.globaltype     __stack_pointer, i32

.functype      asyncify_start_unwind (i32) -> ()
.import_module asyncify_start_unwind, asyncify
.import_name   asyncify_start_unwind, start_unwind

.functype      asyncify_stop_unwind () -> ()
.import_module asyncify_stop_unwind, asyncify
.import_name   asyncify_stop_unwind, stop_unwind

.functype      asyncify_start_rewind (i32) -> ()
.import_module asyncify_start_rewind, asyncify
.import_name   asyncify_start_rewind, start_rewind

.functype      asyncify_stop_rewind () -> ()
.import_module asyncify_stop_rewind, asyncify
.import_name   asyncify_stop_rewind, stop_rewind

# For a suspended stack, the 16 bytes below the stack pointers are used to store some info:
# * -4: payload
# * -8: pointer to the entry point
# * -12: asyncify stack limit
# * -16: asyncify stack pointer
#
# If the entry point is 0, it means that the suspended stack is the main stack, and it not
# entered from fiber_enter.
#
# For a suspended stack created from fiber_enter, the bytes below the specified 16 bytes are used
# as asyncify stack when it is suspended.

.section .text.fiber_enter,"",@

# fiber_enter_impl: fn(StackPointer, usize, fn(StackPointer, usize) -> !) -> SwitchResult
.type fiber_enter_impl, @function
fiber_enter_impl:
    .functype fiber_enter_impl (i32, i32, i32, i32) -> ()
    .local i32

    # Check if we are in the process of rewinding.
    i32.const 0
    i32.load rewinding_stack
    if
        # This is the more complicated case where are are an intermediate frame of rewinding.
        # See the save code at the end of function as well.

        # All our local variables are garbage when rewinding.
        # We need to extract local 0,1,3 from the asyncify stack.

        global.get __stack_pointer
        i32.const 16
        i32.sub
        local.set 4

        # Retrieve the asyncify stack pointer
        local.get 4
        i32.load 0
        i32.const 16
        i32.sub
        local.set 2

        # Decrement the asyncify stack pointer
        local.get 4
        local.get 2
        i32.store 0

        # Load locals
        local.get 2
        i32.load 0
        local.set 0

        local.get 2
        i32.load 4
        local.set 1

        local.get 2
        i32.load 12
        local.set 3
    else

        # Swap argument 0 with __stack_pointer
        local.get 1
        global.get __stack_pointer
        local.set 1
        global.set __stack_pointer

        # If function pointer is 0, then we'll start rewind.
        local.get 3
        i32.const 0
        i32.eq
        if
            global.get __stack_pointer
            i32.const 16
            i32.sub
            local.set 4

            # Store the payload
            local.get 4
            local.get 2
            i32.store 12

            # Store the new stack pointer
            i32.const 0
            local.get 1
            i32.store rewinding_stack

            # Prepare for rewind
            local.get 4
            call asyncify_start_rewind

            # Retrieve the saved function pointer
            local.get 4
            i32.load 8
            local.set 3
        end_if

    end_if

    local.get 1
    local.get 2
    local.get 3
    call_indirect (i32, i32) -> ()

    # The only way that the above call returns is through unwinding.

    global.get __stack_pointer
    i32.const 16
    i32.sub
    local.set 4

    # But we can't stop unwind yet, we need to make sure if we are actually
    # the target.

    i32.const 0
    i32.load unwinding_stack
    local.get 1
    i32.eq
    if
        # Okay, we are indeed the target

        i32.const 0
        i32.const 0
        i32.store unwinding_stack

        call asyncify_stop_unwind

        # Store the function pointer into suspended stack.
        local.get 4
        local.get 3
        i32.store 8

        # Store SwitchResult.0
        local.get 0
        global.get __stack_pointer
        i32.store 0

        # Store SwitchResult.1
        local.get 0
        local.get 4
        i32.load 12
        i32.store 4
        
        # Restore __stack_pointer
        local.get 1
        global.set __stack_pointer

    else
    
        # Now, this case is more complicated.
        # We are not the target, so we must not stop unwinding. This means we need to save our
        # states (local 0,1,3) as well and recover them when rewinding happens.
        
        # What's worse is that we have to save the state on the asyncify stack; the global
        # __asyncify_data is not visible to us. Luckily, we actually know that asyncify_data is
        # __stack_pointer - 16!

        # Fetch asyncify stack pointer
        local.get 4
        i32.load 0
        local.set 2

        # Asyncify stack overflow check
        local.get 2
        i32.const 16
        i32.add
        local.get 4
        i32.load 4
        i32.gt_u
        if
            unreachable
        end_if

        # Save locals
        local.get 2
        local.get 0
        i32.store 0

        local.get 2
        local.get 1
        i32.store 4
        
        local.get 2
        local.get 3
        i32.store 12

        # Increment asyncify stack pointer and we're done!
        local.get 4
        local.get 2
        i32.const 16
        i32.add
        i32.store 0
    end_if

    end_function

# fiber_enter: fn(StackPointer, usize, fn(StackPointer, usize) -> !) -> SwitchResult
# Enter a fresh stack and call the supplied function
.global fiber_enter
.type fiber_enter, @function
fiber_enter:
    .functype fiber_enter (i32, i32, i32, i32) -> ()
    .local i32

    local.get 0
    local.get 1
    local.get 2
    local.get 3
    call fiber_enter_impl

    # Given the alignment of stack, this can never be called -- of course the compiler don't know
    # that, so this will remain. Asyncify will therefore treat as this function as unwindable.
    # This is only necessary when a yield crosses nested generators.
    i32.const 0
    i32.load rewinding_stack
    i32.const 1
    i32.eq
    if
        call asyncify_stop_rewind
    end_if
    end_function

.section .text.fiber_switch_enter,"",@

# fiber_switch_enter: fn(StackPointer, usize) -> SwitchResult
.global fiber_switch_enter
.type fiber_switch_enter, @function
fiber_switch_enter:
    .functype fiber_switch_enter (i32, i32, i32) -> ()

    local.get 0
    local.get 1
    local.get 2
    i32.const 0
    call fiber_enter_impl

    # Given the alignment of stack, this can never be called -- of course the compiler don't know
    # that, so this will remain. Asyncify will therefore treat as this function as unwindable.
    # This is only necessary when a yield crosses nested generators.
    i32.const 0
    i32.load rewinding_stack
    i32.const 1
    i32.eq
    if
        call asyncify_stop_rewind
    end_if
    end_function

.section .text.fiber_switch_leave,"",@

# fiber_switch_leave: fn(StackPointer, usize) -> SwitchResult
.global fiber_switch_leave
.type fiber_switch_leave, @function
fiber_switch_leave:
    .functype fiber_switch_leave (i32, i32, i32) -> ()
    .local i32

    global.get __stack_pointer
    i32.const 16
    i32.sub
    local.set 3

    i32.const 0
    i32.load rewinding_stack
    if
        # In this case we are rewinding in, meaning that we are being resumed.

        # Stop asyncify from rewinding
        call asyncify_stop_rewind

        # Load the updated stack pointer
        local.get 0
        i32.const 0
        i32.load rewinding_stack
        i32.store 0

        i32.const 0
        i32.const 0
        i32.store rewinding_stack

        # Load the payload
        local.get 0
        local.get 3
        i32.load 12
        i32.store 4

    else

        # In this case we are suspending, so need to trigger an unwinding.

        # Store the target stack pointer
        i32.const 0
        local.get 1
        i32.store unwinding_stack

        # Store the payload
        local.get 3
        local.get 2
        i32.store 12

        # Store the asyncify stack pointer
        local.get 3
        global.get __stack_pointer
        i32.const 65552
        i32.sub
        i32.store 0

        # Store the asyncify stack limit
        local.get 3
        local.get 3
        i32.store 4

        # Start unwinding
        local.get 3
        call asyncify_start_unwind

    end_if

    end_function

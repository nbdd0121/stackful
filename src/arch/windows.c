#include <windows.h>
#include <intrin.h>

__declspec(thread) int active_fibers = 0;
__declspec(thread) void *switching_fiber = 0;
__declspec(thread) void *switching_payload = 0;

typedef struct {
    void *fiber;
    void *payload;
} switch_result;

typedef void (*fiber_func)(void*, void*);

struct enter_payload {
    fiber_func func;
    void *actual_payload;
};

static void fiber_proc(void *param) {
    (void)param;
    struct enter_payload *payload = (struct enter_payload *)switching_payload;
    payload->func(switching_fiber, payload->actual_payload);
}

static switch_result fiber_switch(void *fiber, void *payload) {
    switching_fiber = GetCurrentFiber();
    switching_payload = payload;
    SwitchToFiber(fiber);
    switch_result ret = {
        .fiber = switching_fiber,
        .payload = switching_payload,
    };
    return ret;
}

void* fiber_create() {
    if (active_fibers == 0) {
        ConvertThreadToFiber(0);
    }
    active_fibers += 1;
    return CreateFiber(0x200000, fiber_proc, 0);
}

void fiber_destroy(void *fiber) {
    active_fibers -= 1;
    DeleteFiber(fiber);

    if (active_fibers == 0) {
        ConvertFiberToThread();
    }
}

switch_result fiber_enter(void *fiber, void *payload, fiber_func func) {
    struct enter_payload enter_payload = {
        .func = func,
        .actual_payload = payload,
    };
    return fiber_switch(fiber, &enter_payload);
}

switch_result fiber_switch_enter(void *fiber, void *payload) {
    return fiber_switch(fiber, payload);
}

switch_result fiber_switch_leave(void *fiber, void *payload) {
    return fiber_switch(fiber, payload);
}

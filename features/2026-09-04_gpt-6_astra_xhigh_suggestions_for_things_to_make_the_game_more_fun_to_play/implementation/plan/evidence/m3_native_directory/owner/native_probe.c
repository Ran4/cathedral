/* Diagnostic only: pinned x86_64 GNU libc; never linked into production.
 * malloc calls delegate directly to glibc, so dlsym/bootstrap cannot recurse.
 * Directory aliases each call a resolved RTLD_NEXT address exactly once.
 * Counters are TLS, armed only by one ignored Rust test, and allocate nothing.
 */
#define _GNU_SOURCE
#include <dirent.h>
#include <dlfcn.h>
#include <errno.h>
#include <malloc.h>
#include <stdint.h>
#include <stddef.h>
#include <unistd.h>

extern void *__libc_malloc(size_t);
extern void __libc_free(void *);
struct stats {
    uint64_t opens, successful, reads, closes, allocations, frees;
    uint64_t live, peak_live, request_peak, usable_peak, mismatches;
    uint64_t read_allocations, close_allocations, minimum_budget;
};
static DIR *(*next_opendir)(const char *);
static struct dirent64 *(*next_readdir64)(DIR *);
static struct dirent *(*next_readdir)(DIR *);
static int (*next_closedir)(DIR *);
static __thread struct stats state;
static __thread int armed, phase, failure_mode;
static __thread void *allocation;
static __thread uint64_t (*budget_read)(void *);
static __thread void *budget_context;

__attribute__((constructor)) static void resolve(void) {
    next_opendir = dlsym(RTLD_NEXT, "opendir");
    next_readdir64 = dlsym(RTLD_NEXT, "readdir64");
    next_readdir = dlsym(RTLD_NEXT, "readdir");
    next_closedir = dlsym(RTLD_NEXT, "closedir");
    if (!next_opendir || !next_readdir64 || !next_readdir || !next_closedir)
        _exit(120);
}
static void sample_budget(void) {
    uint64_t bytes = budget_read(budget_context);
    if (bytes < state.minimum_budget) state.minimum_budget = bytes;
}
void cathedral_native_begin(int mode, uint64_t (*read)(void *), void *context) {
    if (armed || allocation || !read) _exit(121);
    state = (struct stats){ .minimum_budget = UINT64_MAX };
    budget_read = read;
    budget_context = context;
    failure_mode = mode;
    armed = 1;
}
void cathedral_native_end(struct stats *out) {
    *out = state;
    armed = 0;
    budget_read = NULL;
    budget_context = NULL;
    /* A leaked native object is a test failure, not silently discarded state. */
    if (allocation || state.live) _exit(122);
}
void *malloc(size_t bytes) {
    if (armed && phase == 1 && failure_mode == 1) {
        failure_mode = 0;
        state.allocations++;
        if (bytes > state.request_peak) state.request_peak = bytes;
        errno = ENOMEM;
        return NULL;
    }
    void *ptr = __libc_malloc(bytes);
    if (armed && phase) {
        state.allocations++;
        if (bytes > state.request_peak) state.request_peak = bytes;
        if (phase == 2) state.read_allocations++;
        if (phase == 3) state.close_allocations++;
        if (ptr) {
            size_t usable = malloc_usable_size(ptr);
            if (usable > state.usable_peak) state.usable_peak = usable;
            if (allocation) state.mismatches++;
            allocation = ptr;
            state.live++;
            if (state.live > state.peak_live) state.peak_live = state.live;
        }
    }
    return ptr;
}
void free(void *ptr) {
    if (armed && ptr && ptr == allocation) {
        if (phase != 3) state.mismatches++;
        state.frees++;
        state.live--;
        allocation = NULL;
    }
    __libc_free(ptr);
}
DIR *opendir(const char *name) {
    if (!armed) return next_opendir(name);
    sample_budget();
    state.opens++;
    if (phase || allocation) state.mismatches++;
    phase = 1;
    DIR *dir = next_opendir(name);
    phase = 0;
    if (dir) {
        state.successful++;
        if ((void *)dir != allocation) state.mismatches++;
    } else if (allocation) state.mismatches++;
    return dir;
}
static int before_read(void) {
    if (!armed) return 0;
    sample_budget();
    state.reads++;
    if (phase) state.mismatches++;
    if (failure_mode == 2) {
        failure_mode = 0;
        errno = EIO;
        return 1;
    }
    phase = 2;
    return 0;
}
struct dirent64 *readdir64(DIR *dir) {
    if (before_read()) return NULL;
    struct dirent64 *entry = next_readdir64(dir);
    if (armed) phase = 0;
    return entry;
}
struct dirent *readdir(DIR *dir) {
    if (before_read()) return NULL;
    struct dirent *entry = next_readdir(dir);
    if (armed) phase = 0;
    return entry;
}
int closedir(DIR *dir) {
    if (!armed) return next_closedir(dir);
    sample_budget();
    state.closes++;
    if (phase || (void *)dir != allocation) state.mismatches++;
    phase = 3;
    int result = next_closedir(dir);
    phase = 0;
    if (allocation || state.live) state.mismatches++;
    sample_budget();
    return result;
}

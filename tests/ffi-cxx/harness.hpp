/*
 * What every `ffi-cxx` module's `test.cpp` is built on.
 *
 * A module is a C++23 program of its own that includes the umbrella header and nothing else from
 * this layer, so what it needs said is how a check is made, how a fixture is read out of the
 * environment, and how the program ends on the checks. Everything is `inline`, so a module that
 * includes this once has the counters to itself and a helper it does not call is not an unused
 * function.
 */

#ifndef ROROLALA_FFI_CXX_HARNESS_HPP
#define ROROLALA_FFI_CXX_HARNESS_HPP

#include <cstddef>
#include <cstdio>
#include <cstdlib>
#include <filesystem>
#include <string>

namespace harness {

/* How many checks have failed so far. */
inline int failures = 0;

/* Counts one check, naming it when it did not hold. */
inline void want(bool held, const char *what) {
    if (!held) {
        std::fprintf(stderr, "FAIL: %s\n", what);
        ++failures;
    }
}

/* The same, for a name built at run time. */
inline void want(bool held, const std::string &what) { want(held, what.c_str()); }

/* One variable's value, or nothing when the run did not set it. */
inline std::string environment(const char *name) {
#if defined(_WIN32)
    // The MSVC runtime marks `getenv` deprecated and the suite is compiled by clang against its
    // headers, so the Windows spelling is used here and nowhere else. A suite that runs there is
    // not one this layer promises, but a headers check still compiles it.
    char *value = nullptr;
    std::size_t length = 0;
    if (_dupenv_s(&value, &length, name) != 0 || value == nullptr) {
        return {};
    }
    std::string out{value};
    std::free(value);
    return out;
#else
    const char *value = std::getenv(name);
    return value == nullptr ? std::string{} : std::string{value};
#endif
}

/* A fixture path the runner names in the environment, or nothing when it names none. */
inline std::filesystem::path fixture(const char *name) {
    const auto value = environment(name);
    return value.empty() ? std::filesystem::path{} : std::filesystem::path{value};
}

/* A fixture value the runner names in the environment, or nothing when it names none. */
inline std::string text(const char *name) { return environment(name); }

/* Ends the program on the checks made so far: nothing said, and nothing failed. */
inline int finish() {
    if (failures != 0) {
        std::fprintf(stderr, "%d checks answered as they should not\n", failures);
        return 1;
    }

    std::printf("answered as it should\n");
    return 0;
}

} // namespace harness

#endif

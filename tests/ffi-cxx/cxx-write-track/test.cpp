/*
 * A write across the C++ layer: the lock a reading hands out, a plan made and carried out, and the
 * cancellation that stops a call before it starts.
 *
 * The file is inside the fixture Workspace, so the run needs no network: the store, the index and
 * the Layout are all here. The plan and the run are two calls, as the native side has them: the
 * plan names the subjects and the digests, and the run repeats them and compares.
 */

#include <fstream>
#include <string>
#include <vector>

#include <rola_plus_plus.hpp>

#include "harness.hpp"

int main() {
    using harness::want;

    const auto directory = harness::fixture("ROLA_WORKSPACE");
    auto scope = rola::WorkspaceScope::open(directory);
    want(scope.has_value(), "the fixture Workspace is found");
    if (!scope) {
        return harness::finish();
    }

    auto reading = rola::read(*scope);
    want(reading.has_value(), "the Workspace reads");
    if (!reading) {
        return harness::finish();
    }

    rola::WorkspaceWriter writer{*reading};

    // The lock is held for as long as the guard lives, and the same reading says so.
    {
        auto guard = writer.lock();
        want(guard.has_value(), "the Workspace is locked");
        want(reading->locked(), "holding the guard is holding the lock");
    }
    want(!reading->locked(), "giving the guard back gives the lock back");

    // A file the Work holds, named the way a Layout names one. The Workspace does not carry the
    // directory yet, so the fixture makes it before the plan is asked for.
    std::filesystem::create_directories(directory / "art");
    {
        std::ofstream file{directory / "art" / "cxx.psd", std::ios::binary};
        want(static_cast<bool>(file), "the file a plan names is written");
        file << "one";
    }

    rola::TrackRequest request{};
    request.files = {"art/cxx.psd"};

    auto plan = writer.plan_track(request);
    want(plan.has_value(), "a plan is made for a file in the Work");
    if (plan) {
        want(!plan->subjects.empty(), "the plan names the file as a subject");
        want(plan->agreed, "the plan carries the agreement the run checks");

        std::vector<rola::TrackMessage> messages;
        messages.reserve(plan->subjects.size());
        for (const auto &subject : plan->subjects) {
            messages.push_back(rola::TrackMessage{.path = subject.path, .text = "first pass"});
        }

        auto done = writer.apply_track(request, *plan, messages);
        want(done.has_value(), "the plan is carried out");
        if (done) {
            want(!done->items.empty(), "the run records the file");
            want(done->refused.empty(), "the run refuses nothing");
        }
    }

    // A cancellation asked for before the call is answered at the call's first step.
    auto stopped = rola::Cancellation::make();
    want(stopped.valid(), "a cancellation handle is made");
    stopped.cancel();
    want(stopped.cancelled(), "a cancellation that was asked to stop says so");

    auto cancelled = writer.plan_track(request, nullptr, &stopped);
    want(!cancelled.has_value(), "a plan asked for after the stop is refused");
    if (!cancelled) {
        want(cancelled.error().kind() == rola::ErrorKind::cancelled,
             "the refusal is cancelled");
    }

    return harness::finish();
}

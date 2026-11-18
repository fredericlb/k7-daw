// SeparationBudget_test.cpp — a phone's thread count, what a cut needs, and the refusal when memory falls short.

#include "SeparationBudget.h"

#include "test_harness.h"

using namespace k7::separation;

namespace {

void aPhoneTakesHalfItsCores()
{
    CHECK(threadCount(6, true) == 3);
    CHECK(threadCount(1, true) == 1);
    CHECK(threadCount(6, false) == 1);
}

void aCutNeedsMoreForALongerSong()
{
    CHECK(bytesNeeded(160u << 20, 44100u * 240) > bytesNeeded(160u << 20, 44100u * 10));
    CHECK(bytesNeeded(160u << 20, 0) > (std::size_t(320) << 20));
}

void aCutIsRefusedWhenMemoryFallsShort()
{
    const auto need = bytesNeeded(160u << 20, 44100u * 240);
    CHECK(memoryRefusal(need, need).empty());
    CHECK(memoryRefusal(need + 1, need).empty());
    CHECK(!memoryRefusal(need - 1, need).empty());
    CHECK(memoryRefusal(need - 1, need).find("MB") != std::string::npos);
}

void aRaisedFloorRefusesWhatFits()
{
    CHECK(memoryRefusal(std::size_t(2) << 30, 1u << 20).empty());
    CHECK(!memoryRefusal(std::size_t(2) << 30, 1u << 20, std::size_t(4) << 30).empty());
}

}  // namespace

void runSeparationBudgetTests()
{
    RUN(aPhoneTakesHalfItsCores);
    RUN(aCutNeedsMoreForALongerSong);
    RUN(aCutIsRefusedWhenMemoryFallsShort);
    RUN(aRaisedFloorRefusesWhatFits);
}

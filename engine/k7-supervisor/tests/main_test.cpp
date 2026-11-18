// main_test.cpp — the supervisor's unit tests, one binary: each file registers a `run*Tests()` that
// RUNs its cases. No device, no network, no engine — what needs those is exercised by hand.

#include "test_harness.h"

void runUrlTests();
void runSerialQueueTests();
void runSettingsTests();
void runHostProtocolTests();
void runAudioPolicyTests();
void runPerformanceTests();
void runMachineJobsTests();
void runRtcFramesTests();
void runSeparationAudioTests();
void runSeparationBudgetTests();
void runSeparationDecodeTests();
void runSlotsTests();
void runHostLockTests();
void runRegistryTests();
void runTlsTests();
void runInstrumentsTests();
void runPathMapTests();
void runPidFileTests();
void runJobSlotsTests();
void runLogTests();
void runBackpressureTests();
void runHttpTests();
void runCrashLoopTests();
void runEnginePortsTests();

int main()
{
    runUrlTests();
    runSerialQueueTests();
    runSettingsTests();
    runHostProtocolTests();
    runAudioPolicyTests();
    runPerformanceTests();
    runMachineJobsTests();
    runRtcFramesTests();
    runSeparationAudioTests();
    runSeparationBudgetTests();
    runSeparationDecodeTests();
    runSlotsTests();
    runHostLockTests();
    runRegistryTests();
    runTlsTests();
    runInstrumentsTests();
    runPathMapTests();
    runPidFileTests();
    runJobSlotsTests();
    runLogTests();
    runBackpressureTests();
    runHttpTests();
    runCrashLoopTests();
    runEnginePortsTests();
    TEST_MAIN_END();
}

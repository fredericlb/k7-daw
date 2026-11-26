# Engine unit tests — the JUCE-free data layer only.
#
# Deliberately does NOT link studio_engine (which pulls in Tracktion/JUCE). Instead
# it compiles the two plain-data TUs straight into the test binary, so the tests build
# in seconds and run headless with no audio device — exactly what docs/design.md means
# by "diffable and unit-testable without an audio device".

add_executable(studio_engine_tests
    ProjectDiff_test.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine/ProjectDoc.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine/ProjectDiff.cpp
    # index() names a duplicate id rather than dropping it in silence. Warnings.cpp is JUCE-free, so
    # this keeps the "builds in seconds, no audio device" property the comment above is about.
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine/Warnings.cpp)

# The data-layer headers (ProjectDoc.h / ProjectDiff.h) live in ../engine.
target_include_directories(studio_engine_tests PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine)

add_test(NAME studio_engine_tests COMMAND studio_engine_tests)

# --- Warnings tests -----------------------------------------------------------
# The standing warnings a late client is told: Warnings.cpp is JUCE-free, so this too builds in seconds.
add_executable(studio_warnings_tests
    Warnings_test.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine/Warnings.cpp)
target_include_directories(studio_warnings_tests PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/../engine)
add_test(NAME studio_warnings_tests COMMAND studio_warnings_tests)

# --- DocJson tests ------------------------------------------------------------
# The wire's bounds, in their own fast binary: DocJson.cpp is JUCE-free and needs only nlohmann.
add_executable(studio_docjson_tests
    DocJson_test.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine/PathRoots.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/../server/DocJson.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine/ProjectDoc.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine/Warnings.cpp)
target_include_directories(studio_docjson_tests PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/../engine)
target_link_libraries(studio_docjson_tests PRIVATE nlohmann_json::nlohmann_json)
add_test(NAME studio_docjson_tests COMMAND studio_docjson_tests)

# --- Wire JSON tests ----------------------------------------------------------
# nlohmann's printer held to the fixture the Rust server's printer is held to: nlohmann only, fast.
add_executable(studio_wirejson_tests WireJson_test.cpp)
target_link_libraries(studio_wirejson_tests PRIVATE nlohmann_json::nlohmann_json)
target_compile_definitions(studio_wirejson_tests PRIVATE
    K7_WIRE_FIXTURE="${CMAKE_CURRENT_SOURCE_DIR}/../../../k7-daw-backend/packages/k7-runtime-server/fixtures/nlohmann_dump.txt")
add_test(NAME studio_wirejson_tests COMMAND studio_wirejson_tests)

# --- Pairing tests ------------------------------------------------------------
# Who counts as the machine, and the store of paired LAN devices: Pairing.cpp is JUCE-free and needs only nlohmann.
add_executable(studio_pairing_tests
    Pairing_test.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/../server/Pairing.cpp)
target_include_directories(studio_pairing_tests PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/../server)
target_link_libraries(studio_pairing_tests PRIVATE nlohmann_json::nlohmann_json)
add_test(NAME studio_pairing_tests COMMAND studio_pairing_tests)

# --- Supersede tests ----------------------------------------------------------
# Which queued project, tracks and parameter jobs a newer message skips: Supersede.h is header-only and JUCE-free.
add_executable(studio_supersede_tests Supersede_test.cpp)
target_include_directories(studio_supersede_tests PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/../server)
add_test(NAME studio_supersede_tests COMMAND studio_supersede_tests)

# --- Held notes tests ---------------------------------------------------------
# What a socket that drops mid-note leaves to release: HeldNotes.h is header-only and JUCE-free.
add_executable(studio_heldnotes_tests HeldNotes_test.cpp)
target_include_directories(studio_heldnotes_tests PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/../server)
add_test(NAME studio_heldnotes_tests COMMAND studio_heldnotes_tests)

# Which read-only queries already wait on the message thread: PendingQueries.h is header-only and JUCE-free.
add_executable(studio_pendingqueries_tests PendingQueries_test.cpp)
target_include_directories(studio_pendingqueries_tests PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/../server)
add_test(NAME studio_pendingqueries_tests COMMAND studio_pendingqueries_tests)

# --- Session registry tests ---------------------------------------------------
# A THIRD fast binary, kept apart only because each test file owns a main(). SessionRegistry.h is
# header-only and JUCE-free, so this links nothing and shares the "builds in seconds" property.
add_executable(studio_session_tests SessionRegistry_test.cpp)
target_include_directories(studio_session_tests PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine)
add_test(NAME studio_session_tests COMMAND studio_session_tests)

# --- Turntable loop tests -----------------------------------------------------
# A FOURTH fast binary, same reason as the one above: TurntableLoop.h is header-only and JUCE-free,
# so the loop's arithmetic is tested without linking Tracktion. The PLUGIN around it still is not
# — applyToBuffer needs a live te::Edit, which is what scripts/turntable.ts drives instead.
add_executable(studio_turntable_tests TurntableLoop_test.cpp)
target_include_directories(studio_turntable_tests PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine)
add_test(NAME studio_turntable_tests COMMAND studio_turntable_tests)

# --- Warp marker tests --------------------------------------------------------
# Another fast binary, for the same reason: WarpMarkers.h is header-only and JUCE-free, so which markers reach
# Tracktion is tested without linking it. That the kept ones TIME the sound is scripts/warp-points.ts's.
add_executable(studio_warp_tests WarpMarkers_test.cpp)
target_include_directories(studio_warp_tests PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine)
add_test(NAME studio_warp_tests COMMAND studio_warp_tests)

# --- Plugin scan tests --------------------------------------------------------
# What the startup scan reuses and drops, and when its progress is said: ScanProgress.h is header-only and JUCE-free.
add_executable(studio_scanprogress_tests ScanProgress_test.cpp)
target_include_directories(studio_scanprogress_tests PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine)
add_test(NAME studio_scanprogress_tests COMMAND studio_scanprogress_tests)

# --- Recorder ring tests ------------------------------------------------------
# A FIFTH fast binary, for the reason the two above give: RecorderTap.h is header-only and
# JUCE-free, so the one property no eye can check — that an overrun is COUNTED rather than
# forgiven — is tested without linking Tracktion. See ../docs/recorder.md. The writer thread rides along.
add_executable(studio_recorder_tests RecorderTap_test.cpp RecorderWriter_test.cpp)
target_include_directories(studio_recorder_tests PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine)
add_test(NAME studio_recorder_tests COMMAND studio_recorder_tests)

# --- Arpeggiator tests --------------------------------------------------------
# A SIXTH fast binary, same reason as the three above: Arpeggiator.h is header-only and JUCE-free,
# so the clock and the walk are tested without linking Tracktion. The PLUGIN around it needs a live
# te::Edit no suite here builds — see ../docs/arpeggiator.md.
add_executable(studio_arp_tests Arpeggiator_test.cpp)
target_include_directories(studio_arp_tests PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine)
add_test(NAME studio_arp_tests COMMAND studio_arp_tests)

# --- Naturalize dither tests -------------------------------------------------
# A fast binary like the ones above: NaturalizeDither.h is header-only and JUCE-free, so Quick
# Master's output stage is tested without linking Tracktion. See ../docs/quickMaster.md.
add_executable(studio_dither_tests NaturalizeDither_test.cpp)
target_include_directories(studio_dither_tests PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine)
add_test(NAME studio_dither_tests COMMAND studio_dither_tests)

# …and its HF limit, To Vinyl 4's acceleration limiter: header-only and JUCE-free for the same reason.
add_executable(studio_hflimit_tests AccelerationLimiter_test.cpp)
target_include_directories(studio_hflimit_tests PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine)
add_test(NAME studio_hflimit_tests COMMAND studio_hflimit_tests)

# --- Note clock tests ---------------------------------------------------------
# A SEVENTH fast binary, for the reason the arpeggiator's gives: NoteClock.h is header-only and
# JUCE-free, so the held set, the clock and the gate are tested without linking Tracktion. The
# GRAPH nodes over it need a MidiMessageArray and are in studio_midigraph_tests — ../docs/midiGraph.md.
add_executable(studio_noteclock_tests NoteClock_test.cpp)
target_include_directories(studio_noteclock_tests PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine)
add_test(NAME studio_noteclock_tests COMMAND studio_noteclock_tests)

# --- Callback watchdog tests --------------------------------------------------
# Another fast binary, for the reason the ones above give: CallbackWatchdog.h is header-only and
# JUCE-free, so WHEN a stalled device is reopened is tested without linking Tracktion. The reopen
# itself needs a live device no suite here opens — ../docs/audioDevice.md.
add_executable(studio_watchdog_tests CallbackWatchdog_test.cpp)
target_include_directories(studio_watchdog_tests PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine)
add_test(NAME studio_watchdog_tests COMMAND studio_watchdog_tests)

# --- Hang watch tests ---------------------------------------------------------
# Fast for the same reason: HangWatch.h is header-only and JUCE-free, so WHEN a hung engine leaves is tested without Tracktion (../docs/hangWatch.md).
add_executable(studio_hangwatch_tests HangWatch_test.cpp)
target_include_directories(studio_hangwatch_tests PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine)
add_test(NAME studio_hangwatch_tests COMMAND studio_hangwatch_tests)

# --- Render pool tests --------------------------------------------------------
# Fast for the same reason: RenderPool.h is header-only and JUCE-free, the settings read from the environment (../docs/renderPool.md).
add_executable(studio_renderpool_tests RenderPool_test.cpp)
target_include_directories(studio_renderpool_tests PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine)
add_test(NAME studio_renderpool_tests COMMAND studio_renderpool_tests)

# --- MIDI graph tests ---------------------------------------------------------
# A SECOND binary, because these need Tracktion's MidiMessageArray and so must link the engine — which is exactly
# what the target above avoids. Keeping them apart preserves the fast tests' "builds in seconds, no audio device"
# property; this one is slower and is why it is opt-in the same way.
add_executable(studio_midigraph_tests MidiGraph_test.cpp)
# Tracktion is linked PRIVATE by studio_engine — the library hides JUCE from its consumers on purpose. But
# DspGraph.h is an INTERNAL header and does include it, so a test that includes it has to link Tracktion itself,
# with the same headless defines the engine compiles under.
target_link_libraries(studio_midigraph_tests PRIVATE
    studio_engine
    tracktion::tracktion_engine)
target_include_directories(studio_midigraph_tests PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine)
target_compile_definitions(studio_midigraph_tests PRIVATE
    JUCE_MODAL_LOOPS_PERMITTED=1
    JUCE_USE_CURL=0
    JUCE_WEB_BROWSER=0
    DONT_SET_USING_JUCE_NAMESPACE=1)
add_test(NAME studio_midigraph_tests COMMAND studio_midigraph_tests)
# `slow` is what keeps this out of `npm run test`: linking Tracktion can cost minutes — ../../../../docs/tests.md.
set_tests_properties(studio_midigraph_tests PROPERTIES LABELS slow)


# --- DSP graph tests ----------------------------------------------------------
# Same deal as the MIDI ones: DspGraph.h is internal and includes Tracktion, so this links it too. Kept apart
# from MidiGraph_test.cpp only because each owns a main().
add_executable(studio_dspgraph_tests DspGraph_test.cpp)
target_link_libraries(studio_dspgraph_tests PRIVATE
    studio_engine
    tracktion::tracktion_engine)
target_include_directories(studio_dspgraph_tests PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine)
target_compile_definitions(studio_dspgraph_tests PRIVATE
    JUCE_MODAL_LOOPS_PERMITTED=1
    JUCE_USE_CURL=0
    JUCE_WEB_BROWSER=0
    DONT_SET_USING_JUCE_NAMESPACE=1)
add_test(NAME studio_dspgraph_tests COMMAND studio_dspgraph_tests)
# Slow for the same reason as the MIDI one, and opt-in the same way.
set_tests_properties(studio_dspgraph_tests PROPERTIES LABELS slow)

# --- Engine teardown tests ----------------------------------------------------
# A whole EngineHost, built and destroyed with its edits rolling: it links Tracktion to ask JUCE's Desktop
# whether the screen is still held, so it is slow like the two above (../docs/engineHost.md).
add_executable(studio_teardown_tests EngineTeardown_test.cpp)
target_link_libraries(studio_teardown_tests PRIVATE
    studio_engine
    tracktion::tracktion_engine)
target_include_directories(studio_teardown_tests PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine)
target_compile_definitions(studio_teardown_tests PRIVATE
    JUCE_MODAL_LOOPS_PERMITTED=1
    JUCE_USE_CURL=0
    JUCE_WEB_BROWSER=0
    DONT_SET_USING_JUCE_NAMESPACE=1)
add_test(NAME studio_teardown_tests COMMAND studio_teardown_tests)
set_tests_properties(studio_teardown_tests PROPERTIES LABELS slow)

# --- Render loop tests --------------------------------------------------------
# A looped clip bounced offline: it links Tracktion and renders for real, so it is slow like the teardown suite.
add_executable(studio_renderloop_tests RenderLoop_test.cpp)
target_link_libraries(studio_renderloop_tests PRIVATE
    studio_engine
    tracktion::tracktion_engine)
target_include_directories(studio_renderloop_tests PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine)
target_compile_definitions(studio_renderloop_tests PRIVATE
    JUCE_MODAL_LOOPS_PERMITTED=1
    JUCE_USE_CURL=0
    JUCE_WEB_BROWSER=0
    DONT_SET_USING_JUCE_NAMESPACE=1)
add_test(NAME studio_renderloop_tests COMMAND studio_renderloop_tests)
set_tests_properties(studio_renderloop_tests PROPERTIES LABELS slow)

# --- Pattern lane tests -------------------------------------------------------
# Two fast binaries for PatternPlugin's two JUCE-free halves: the schedule between the worker and the
# audio thread (header-only), and the QuickJS host with the embedded Strudel bundle (studio_strudel).
# The plugin around them needs a live te::Edit — its proof is an offline render, ../docs/patternPlugin.md.
add_executable(studio_pattern_tests PatternSchedule_test.cpp)
target_include_directories(studio_pattern_tests PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine)
add_test(NAME studio_pattern_tests COMMAND studio_pattern_tests)

add_executable(studio_samplebanks_tests SampleBanks_test.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine/PathRoots.cpp)
target_include_directories(studio_samplebanks_tests PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine)
add_test(NAME studio_samplebanks_tests COMMAND studio_samplebanks_tests)

# Which paths a document may make the engine read or write: PathRoots.cpp is JUCE-free (docs/pathRoots.md).
add_executable(studio_pathroots_tests PathRoots_test.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine/PathRoots.cpp)
target_include_directories(studio_pathroots_tests PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine)
add_test(NAME studio_pathroots_tests COMMAND studio_pathroots_tests)

# Where Settings.xml and the scan cache go: SettingsDir.cpp is JUCE-free (docs/settingsDir.md).
add_executable(studio_settingsdir_tests SettingsDir_test.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine/SettingsDir.cpp)
target_include_directories(studio_settingsdir_tests PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine)
add_test(NAME studio_settingsdir_tests COMMAND studio_settingsdir_tests)

add_executable(studio_strudel_tests StrudelHost_test.cpp)
target_link_libraries(studio_strudel_tests PRIVATE studio_strudel)
add_test(NAME studio_strudel_tests COMMAND studio_strudel_tests)

# --- Eurorack core tests ------------------------------------------------------
# FAST like the others: the cores are JUCE-free by construction (the firmware's macros forbid sharing
# a TU with JUCE), so their pitch and their voice and envelope rules are tested without Tracktion.
# The plugins around them — the resampler, the MIDI cut, the sidechain — are not; ../docs/eurorack.md.
add_executable(studio_eurorack_tests Eurorack_test.cpp)
target_include_directories(studio_eurorack_tests PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine/eurorack)
target_link_libraries(studio_eurorack_tests PRIVATE studio_eurorack)
add_test(NAME studio_eurorack_tests COMMAND studio_eurorack_tests)

# --- Amp model tests ------------------------------------------------------------
# The RTNeural half of AmpModel, JUCE-free like the fast binaries above; references from data/ampmodel/gen_fixtures.py.
add_executable(studio_ampmodel_tests AmpModel_test.cpp ${CMAKE_CURRENT_SOURCE_DIR}/../engine/RtNeuralModel.cpp)
target_include_directories(studio_ampmodel_tests PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/../engine)
target_link_libraries(studio_ampmodel_tests PRIVATE studio_rtneural)
target_compile_definitions(studio_ampmodel_tests PRIVATE
    STUDIO_AMPMODEL_FIXTURES="${CMAKE_CURRENT_SOURCE_DIR}/data/ampmodel")
add_test(NAME studio_ampmodel_tests COMMAND studio_ampmodel_tests)

# --- Slot footswitch tests ----------------------------------------------------
# What a press of the slot's footswitch bypasses: SlotFootswitch.h is header-only and JUCE-free.
add_executable(studio_slotfootswitch_tests SlotFootswitch_test.cpp)
target_include_directories(studio_slotfootswitch_tests PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/../engine)
add_test(NAME studio_slotfootswitch_tests COMMAND studio_slotfootswitch_tests)

# --- C ABI tests --------------------------------------------------------------
# The Rust server's door into the host, a whole EngineHost behind it: slow like the teardown suite (engine/capi/k7_engine.h).
add_executable(studio_capi_tests CApi_test.cpp)
target_link_libraries(studio_capi_tests PRIVATE
    studio_engine
    tracktion::tracktion_engine)
target_include_directories(studio_capi_tests PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine)
target_compile_definitions(studio_capi_tests PRIVATE
    JUCE_MODAL_LOOPS_PERMITTED=1
    JUCE_USE_CURL=0
    JUCE_WEB_BROWSER=0
    DONT_SET_USING_JUCE_NAMESPACE=1)
add_test(NAME studio_capi_tests COMMAND studio_capi_tests)
set_tests_properties(studio_capi_tests PROPERTIES LABELS slow)

# --- Rust server link tests ---------------------------------------------------
# Rust's `serve` on the real host, probed with `whoami` over its socket: slow like the C ABI's (engine/capi/k7_server.h).
add_executable(studio_rustserve_tests RustServe_test.cpp)
target_link_libraries(studio_rustserve_tests PRIVATE
    k7_runtime_server
    studio_engine
    tracktion::tracktion_engine
    ixwebsocket
    nlohmann_json::nlohmann_json)
k7_keep_rust_entry(studio_rustserve_tests)
target_include_directories(studio_rustserve_tests PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine)
target_compile_definitions(studio_rustserve_tests PRIVATE
    JUCE_MODAL_LOOPS_PERMITTED=1
    JUCE_USE_CURL=0
    JUCE_WEB_BROWSER=0
    DONT_SET_USING_JUCE_NAMESPACE=1)
add_test(NAME studio_rustserve_tests COMMAND studio_rustserve_tests)
set_tests_properties(studio_rustserve_tests PROPERTIES LABELS slow)

# --- DocReader tests ----------------------------------------------------------
# The plain reader of the document the Rust server validated, held to the golden file its test writes: nlohmann only, fast.
add_executable(studio_docreader_tests
    DocReader_test.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine/DocReader.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine/ProjectDoc.cpp)
target_include_directories(studio_docreader_tests PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/../engine)
target_link_libraries(studio_docreader_tests PRIVATE nlohmann_json::nlohmann_json)
target_compile_definitions(studio_docreader_tests PRIVATE
    K7_GOLDEN_DOC="${CMAKE_CURRENT_SOURCE_DIR}/../../../k7-daw-backend/packages/k7-runtime-server/fixtures/project.validated.json")
add_test(NAME studio_docreader_tests COMMAND studio_docreader_tests)

# --- State area tests ---------------------------------------------------------
# The state area of the C ABI on a whole EngineHost (a real timer's frame), slow like the C ABI's own suite.
add_executable(studio_stateapi_tests StateApi_test.cpp)
target_link_libraries(studio_stateapi_tests PRIVATE
    studio_engine
    tracktion::tracktion_engine)
target_include_directories(studio_stateapi_tests PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine)
target_compile_definitions(studio_stateapi_tests PRIVATE
    JUCE_MODAL_LOOPS_PERMITTED=1
    JUCE_USE_CURL=0
    JUCE_WEB_BROWSER=0
    DONT_SET_USING_JUCE_NAMESPACE=1)
add_test(NAME studio_stateapi_tests COMMAND studio_stateapi_tests)
set_tests_properties(studio_stateapi_tests PROPERTIES LABELS slow)

# --- Plugins area tests -------------------------------------------------------
# The plugins area of the C ABI on a whole EngineHost (the cache, a probe of an unlisted type), slow like the C ABI's own suite.
add_executable(studio_pluginsapi_tests PluginsApi_test.cpp)
target_link_libraries(studio_pluginsapi_tests PRIVATE
    studio_engine
    tracktion::tracktion_engine)
target_include_directories(studio_pluginsapi_tests PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine)
target_compile_definitions(studio_pluginsapi_tests PRIVATE
    JUCE_MODAL_LOOPS_PERMITTED=1
    JUCE_USE_CURL=0
    JUCE_WEB_BROWSER=0
    DONT_SET_USING_JUCE_NAMESPACE=1)
add_test(NAME studio_pluginsapi_tests COMMAND studio_pluginsapi_tests)
set_tests_properties(studio_pluginsapi_tests PROPERTIES LABELS slow)

# --- Plugin replies tests -----------------------------------------------------
# The plugin replies as ServePlugins.cpp builds them, held to the fixture the Rust server's replies are held to too: nlohmann only, fast.
add_executable(studio_pluginreplies_tests PluginReplies_test.cpp)
target_include_directories(studio_pluginreplies_tests PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/../engine)
target_link_libraries(studio_pluginreplies_tests PRIVATE nlohmann_json::nlohmann_json)
target_compile_definitions(studio_pluginreplies_tests PRIVATE
    K7_PLUGIN_FIXTURE="${CMAKE_CURRENT_SOURCE_DIR}/../../../k7-daw-backend/packages/k7-runtime-server/fixtures/plugin_replies.txt")
add_test(NAME studio_pluginreplies_tests COMMAND studio_pluginreplies_tests)

# --- State frame tests --------------------------------------------------------
# The state frame as Serve.cpp builds it, held to the fixture the Rust server's frame is held to: nlohmann only, fast.
add_executable(studio_stateframe_tests StateFrame_test.cpp)
target_include_directories(studio_stateframe_tests PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/../engine)
target_link_libraries(studio_stateframe_tests PRIVATE nlohmann_json::nlohmann_json)
target_compile_definitions(studio_stateframe_tests PRIVATE
    K7_STATE_FIXTURE="${CMAKE_CURRENT_SOURCE_DIR}/../../../k7-daw-backend/packages/k7-runtime-server/fixtures/state_frame.txt")
add_test(NAME studio_stateframe_tests COMMAND studio_stateframe_tests)

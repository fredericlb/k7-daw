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

# --- Wire JSON tests ----------------------------------------------------------
# nlohmann's printer held to the fixture the Rust server's printer is held to: nlohmann only, fast.
add_executable(studio_wirejson_tests WireJson_test.cpp)
target_link_libraries(studio_wirejson_tests PRIVATE nlohmann_json::nlohmann_json)
target_compile_definitions(studio_wirejson_tests PRIVATE
    K7_WIRE_FIXTURE="${CMAKE_CURRENT_SOURCE_DIR}/../../../k7-daw-backend/packages/k7-runtime-server/fixtures/nlohmann_dump.txt")
add_test(NAME studio_wirejson_tests COMMAND studio_wirejson_tests)

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

# --- Machine and render area tests --------------------------------------------
# The machine and render areas of the C ABI on a whole EngineHost, slow like the C ABI's own suite.
add_executable(studio_machineapi_tests MachineApi_test.cpp)
target_link_libraries(studio_machineapi_tests PRIVATE
    studio_engine
    tracktion::tracktion_engine)
target_include_directories(studio_machineapi_tests PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/../engine)
target_compile_definitions(studio_machineapi_tests PRIVATE
    JUCE_MODAL_LOOPS_PERMITTED=1
    JUCE_USE_CURL=0
    JUCE_WEB_BROWSER=0
    DONT_SET_USING_JUCE_NAMESPACE=1)
add_test(NAME studio_machineapi_tests COMMAND studio_machineapi_tests)
set_tests_properties(studio_machineapi_tests PROPERTIES LABELS slow)


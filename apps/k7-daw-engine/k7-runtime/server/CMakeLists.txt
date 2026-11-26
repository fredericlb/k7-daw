# k7-runtime (app) — the WebSocket audio server.
#
# Consumes studio::engine (which hides JUCE/Tracktion), plus IXWebSocket and
# nlohmann/json. It includes no JUCE headers itself.
#
# On Android the same target is a SHARED library, libk7runtime.so, with no main(): JUCE's Oboe
# backend reaches Java, so the engine is loaded into the app rather than spawned, and the app
# calls serve() through android/EngineJni.cpp (docs/android.md). On iOS it is a FRAMEWORK,
# k7runtime.framework, for the same reason and one more — no app may run a program of its own (docs/ios.md).

# The Rust server, a static library cargo builds (docs/link.md in k7-runtime-server's crate).
include(${CMAKE_CURRENT_LIST_DIR}/RustServer.cmake)

set(STUDIO_SERVER_SOURCES
    Serve.cpp
    ServeProject.cpp
    ServePlugins.cpp
    ServeMachine.cpp
    ServeRender.cpp
    ServePairing.cpp
    Pairing.cpp
    AudioSender.cpp
    DocJson.cpp
    WireVersion.cpp
    # The NAM core's objects, on the FINAL link line rather than only inside
    # studio_engine.lib. Its architectures register themselves from a file-scope static
    # that nothing references (see engine/CMakeLists.txt), so as archive members they are
    # dropped and every .nam model fails to load at runtime — from a build that reported
    # nothing wrong. Objects named here are always linked.
    $<TARGET_OBJECTS:studio_nam>)

if(ANDROID)
    add_library(k7-runtime SHARED ${STUDIO_SERVER_SOURCES} android/EngineJni.cpp)
    set_target_properties(k7-runtime PROPERTIES
        OUTPUT_NAME k7runtime
        LIBRARY_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR})
    # Oboe's AAudio/OpenSL ES and JUCE's logging, which the JUCE module sources compiled into
    # studio_engine call but do not link themselves.
    target_link_libraries(k7-runtime PRIVATE android log OpenSLES)
elseif(IOS)
    include(${CMAKE_CURRENT_LIST_DIR}/ios/framework-plist.cmake)
    add_library(k7-runtime SHARED ${STUDIO_SERVER_SOURCES} ios/EngineEmbed.cpp)
    set_target_properties(k7-runtime PROPERTIES
        FRAMEWORK TRUE
        OUTPUT_NAME k7runtime
        MACOSX_FRAMEWORK_IDENTIFIER rocks.k7.daw.runtime
        LIBRARY_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR})
    k7_ios_framework_plist(k7-runtime)
else()
    add_executable(k7-runtime main.cpp ${STUDIO_SERVER_SOURCES})
    # Keep the binary at build/<preset>/k7-runtime.
    set_target_properties(k7-runtime PROPERTIES
        RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR})
endif()

# k7_runtime_server first: its archive needs k7_engine_api from studio::engine, and a linker reads archives in order.
target_link_libraries(k7-runtime PRIVATE
    k7_runtime_server
    studio::engine
    ixwebsocket
    nlohmann_json::nlohmann_json)

k7_keep_rust_entry(k7-runtime)

# The version `whoami` and the heartbeat report, the root's, which k7-runtime's package.json copies (docs/versioning.md).
file(READ "${CMAKE_CURRENT_LIST_DIR}/../package.json" K7_PACKAGE_JSON)
string(JSON K7_BUILD_STAMP GET "${K7_PACKAGE_JSON}" version)
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${CMAKE_CURRENT_LIST_DIR}/../package.json")
set_source_files_properties(WireVersion.cpp PROPERTIES COMPILE_DEFINITIONS "K7_BUILD=\"${K7_BUILD_STAMP}\"")

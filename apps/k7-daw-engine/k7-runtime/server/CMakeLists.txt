# k7-runtime (app) — the engine's entry points over the Rust server, linked as a static library, and studio::engine.
#
# On Android the same target is a SHARED library, libk7runtime.so, with no main(): JUCE's Oboe
# backend reaches Java, so the engine is loaded into the app rather than spawned, and the app
# calls serve through android/EngineJni.cpp (docs/android.md). On iOS it is a FRAMEWORK,
# k7runtime.framework, for the same reason and one more — no app may run a program of its own (docs/ios.md).

# The Rust server, a static library cargo builds (docs/link.md in k7-runtime-server's crate).
include(${CMAKE_CURRENT_LIST_DIR}/RustServer.cmake)

set(STUDIO_SERVER_SOURCES
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
    studio::engine)

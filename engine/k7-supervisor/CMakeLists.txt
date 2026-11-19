# k7-supervisor — the engine machine's supervisor, a native program driven by the tray over NDJSON
# on stdio. Its own project rather than a target of k7-runtime's: building it must never mean
# building Tracktion, JUCE and Faust. See docs/supervisor.md.

cmake_minimum_required(VERSION 3.24)

project(K7Supervisor VERSION 0.0.1 LANGUAGES C CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

if(MSVC)
    add_compile_options(/utf-8)
endif()
# /MP for MSBuild, which runs cl once per file; under Ninja it does nothing, and a compiler cache refuses it.
if(MSVC AND CMAKE_GENERATOR MATCHES "Visual Studio")
    add_compile_options(/MP /FS)
endif()
# The Visual Studio generator defines _MBCS on every target and Ninja does not: the same build under both.
if(MSVC AND NOT CMAKE_GENERATOR MATCHES "Visual Studio")
    add_compile_definitions(_MBCS)
endif()
# Every dependency is linked statically into one executable; position-independent code keeps that legal on Linux.
set(CMAKE_POSITION_INDEPENDENT_CODE ON)
set(BUILD_SHARED_LIBS OFF CACHE BOOL "" FORCE)
# Nothing here is installed, and a dependency's export set would name our mbedTLS targets, which none holds.
set(CMAKE_SKIP_INSTALL_RULES ON)

include(FetchContent)

# Every download SAID, with its progress: a first configure fetches hundreds of MB, and a silent one
# reads as a hang in a CI log. GIT_PROGRESS on each git source below is the other half.
set(FETCHCONTENT_QUIET OFF)

# $STUDIO_FETCHCONTENT_DIR, which CI sets and keeps between runs, holds every download instead of the
# build tree, so a fresh build clones nothing; this project gets its own folder in it. A folder of
# SOURCES only: the job deletes what gets built in it, which differs per preset and per target.
if(NOT "$ENV{STUDIO_FETCHCONTENT_DIR}" STREQUAL "")
    set(FETCHCONTENT_BASE_DIR "$ENV{STUDIO_FETCHCONTENT_DIR}/k7-supervisor" CACHE PATH "" FORCE)
    set(FETCHCONTENT_UPDATES_DISCONNECTED ON)
endif()

# The patch command carries the script's checksum, so a kept source tree re-runs a patch edited since.
file(SHA256 "${CMAKE_CURRENT_SOURCE_DIR}/cmake/patch_ixwebsocket.cmake" STUDIO_PATCH_SUM)

# --- nlohmann/json : every frame and every settings file, the same tag as the engine's --------
# Upstream's release archive for FetchContent: the headers and the CMake files, 0.1 MB against a clone's 186.
FetchContent_Declare(nlohmann_json
    URL      https://github.com/nlohmann/json/releases/download/v3.11.3/json.tar.xz
    URL_HASH SHA256=d6c65aca6b1ed68e7a182f4757257b107ae403032760ed6ef121c9d55e81757d)
set(JSON_BuildTests OFF CACHE BOOL "" FORCE)

# --- mbedTLS : the one TLS stack, statically linked on all three platforms (docs/supervisor.md) ---
# The release archive rather than the git tag: the tag lacks the generated sources a build needs.
FetchContent_Declare(mbedtls
    URL      https://github.com/Mbed-TLS/mbedtls/releases/download/mbedtls-3.6.5/mbedtls-3.6.5.tar.bz2
    URL_HASH SHA256=4a11f1777bb95bf4ad96721cac945a26e04bf19f57d905f241fe77ebeddf46d8)
set(ENABLE_PROGRAMS OFF CACHE BOOL "" FORCE)
set(ENABLE_TESTING OFF CACHE BOOL "" FORCE)
set(MBEDTLS_FATAL_WARNINGS OFF CACHE BOOL "" FORCE)
set(USE_SHARED_MBEDTLS_LIBRARY OFF CACHE BOOL "" FORCE)
set(USE_STATIC_MBEDTLS_LIBRARY ON CACHE BOOL "" FORCE)
set(MBEDTLS_USER_CONFIG_FILE ${CMAKE_CURRENT_SOURCE_DIR}/cmake/mbedtls_user_config.h CACHE FILEPATH "" FORCE)
FetchContent_MakeAvailable(nlohmann_json mbedtls)
# `threading.h` includes "threading_alt.h" by name, so our copy must be on every mbedTLS target's path.
if(WIN32)
    foreach(lib mbedtls mbedx509 mbedcrypto)
        target_include_directories(${lib} PUBLIC ${CMAKE_CURRENT_SOURCE_DIR}/cmake)
    endforeach()
endif()
# The one name libdatachannel asks for, over the three libraries mbedTLS builds.
add_library(k7_mbedtls INTERFACE)
target_link_libraries(k7_mbedtls INTERFACE mbedtls mbedx509 mbedcrypto)
add_library(MbedTLS::MbedTLS ALIAS k7_mbedtls)

# --- miniz : unpacks the instruments the supervisor downloads (Instruments.cpp, docs/instruments.md) ----
# MIT, one C file. The release archive is the amalgamated miniz.c/miniz.h, compiled here as it is.
FetchContent_Declare(miniz
    URL      https://github.com/richgel999/miniz/releases/download/3.0.2/miniz-3.0.2.zip
    URL_HASH SHA256=ada38db0b703a56d3dd6d57bf84a9c5d664921d870d8fea4db153979fb5332c5)
# The archive holds no CMakeLists.txt, so making it available only unpacks it.
FetchContent_MakeAvailable(miniz)
add_library(k7_miniz STATIC ${miniz_SOURCE_DIR}/miniz.c)
target_include_directories(k7_miniz PUBLIC ${miniz_SOURCE_DIR})
set_target_properties(k7_miniz PROPERTIES POSITION_INDEPENDENT_CODE ON)

# --- IXWebSocket : the engine socket, the tunnel, and HTTPS, all over the mbedTLS above ---------
FetchContent_Declare(ixwebsocket
    GIT_REPOSITORY https://github.com/machinezone/IXWebSocket.git
    GIT_PROGRESS   ON
    GIT_TAG        v12.0.1
    GIT_SHALLOW    ON
    # A TLS write left pending is retried at its own length, or a big upload corrupts the tunnel (docs/supervisor.md).
    PATCH_COMMAND  ${CMAKE_COMMAND} -DSTUDIO_PATCH_SUM=${STUDIO_PATCH_SUM} -P ${CMAKE_CURRENT_SOURCE_DIR}/cmake/patch_ixwebsocket.cmake)
set(USE_TLS ON CACHE BOOL "" FORCE)
set(USE_MBED_TLS ON CACHE BOOL "" FORCE)
set(USE_ZLIB OFF CACHE BOOL "" FORCE)
# Told where the fetched mbedTLS is, since its FindMbedTLS looks only for an installed one.
set(MBEDTLS_FOUND TRUE)
set(MBEDTLS_INCLUDE_DIRS ${mbedtls_SOURCE_DIR}/include)
set(MBEDTLS_LIBRARIES mbedtls mbedx509 mbedcrypto)
FetchContent_MakeAvailable(ixwebsocket)
# The define its finder would have set for an mbedTLS 3, skipped because we said it was found.
target_compile_definitions(ixwebsocket PRIVATE IXWEBSOCKET_USE_MBED_TLS_MIN_VERSION_3)

# --- libdatachannel : the WebRTC path to a browser, DTLS over the same mbedTLS (docs/supervisor.md) ---
# Data channels only: no media, no WebSocket of its own, and the three submodules that needs.
FetchContent_Declare(libdatachannel
    GIT_REPOSITORY https://github.com/paullouisageneau/libdatachannel.git
    GIT_PROGRESS   ON
    GIT_TAG        v0.24.5
    GIT_SHALLOW    ON
    GIT_SUBMODULES "deps/plog;deps/usrsctp;deps/libjuice")
set(USE_MBEDTLS ON CACHE BOOL "" FORCE)
set(NO_MEDIA ON CACHE BOOL "" FORCE)
set(NO_WEBSOCKET ON CACHE BOOL "" FORCE)
set(NO_EXAMPLES ON CACHE BOOL "" FORCE)
set(NO_TESTS ON CACHE BOOL "" FORCE)
FetchContent_MakeAvailable(libdatachannel)

# --- ONNX Runtime : stem separation, the prebuilt library per platform, hashes pinned ---------------
# macOS and Linux take Microsoft's NuGet package: the GitHub release archives are gone (404).
# A SHARED library, loaded at run time from beside the executable or the tray's resources. Windows
# takes the DirectML package, which stops at 1.24; a platform with no binary builds without separation.
# Android is one: it is UNIX and aarch64, but the linux-aarch64 build is glibc and loads on no phone.
# iOS takes Microsoft's CocoaPods archive, a STATIC xcframework, linked into the framework (nothing on a phone to load).
set(K7_ORT_URL "")
if(APPLE AND NOT IOS AND (CMAKE_SYSTEM_PROCESSOR STREQUAL "arm64" OR CMAKE_OSX_ARCHITECTURES STREQUAL "arm64"))
    set(K7_ORT_URL https://api.nuget.org/v3-flatcontainer/microsoft.ml.onnxruntime/1.30.0/microsoft.ml.onnxruntime.1.30.0.nupkg)
    set(K7_ORT_HASH 30df119bd9164eaa187666d93147448fdd4a9a1a3caf83f1505daf7af2d1c5d2)
    set(K7_ORT_RID osx-arm64)
elseif(IOS)
    set(K7_ORT_URL https://download.onnxruntime.ai/pod-archive-onnxruntime-c-1.30.0.zip)
    set(K7_ORT_HASH e6f1670c14406fd9f082bb400ab197a9b0a9646058ca6366e440642e2b54a2ea)
elseif(WIN32 AND CMAKE_SIZEOF_VOID_P EQUAL 8)
    set(K7_ORT_URL https://api.nuget.org/v3-flatcontainer/microsoft.ml.onnxruntime.directml/1.24.4/microsoft.ml.onnxruntime.directml.1.24.4.nupkg)
    set(K7_ORT_HASH 57e9f11b73437bef7a309496135d4c1f96b1a8e9ddba60013fa27bfc1d788681)
elseif(UNIX AND NOT APPLE AND NOT ANDROID AND CMAKE_SYSTEM_PROCESSOR MATCHES "x86_64|AMD64")
    set(K7_ORT_URL https://api.nuget.org/v3-flatcontainer/microsoft.ml.onnxruntime/1.30.0/microsoft.ml.onnxruntime.1.30.0.nupkg)
    set(K7_ORT_HASH 30df119bd9164eaa187666d93147448fdd4a9a1a3caf83f1505daf7af2d1c5d2)
    set(K7_ORT_RID linux-x64)
elseif(UNIX AND NOT APPLE AND NOT ANDROID AND CMAKE_SYSTEM_PROCESSOR MATCHES "aarch64|arm64")
    set(K7_ORT_URL https://api.nuget.org/v3-flatcontainer/microsoft.ml.onnxruntime/1.30.0/microsoft.ml.onnxruntime.1.30.0.nupkg)
    set(K7_ORT_HASH 30df119bd9164eaa187666d93147448fdd4a9a1a3caf83f1505daf7af2d1c5d2)
    set(K7_ORT_RID linux-arm64)
endif()
set(K7_ORT_RUNTIME "")
# iOS's archive under a name of its own: one fetch folder serves the Mac's desktop and iOS builds, and a name holds one download.
if(IOS)
    set(K7_ORT_NAME onnxruntime_ios)
else()
    set(K7_ORT_NAME onnxruntime)
endif()
if(K7_ORT_URL)
    FetchContent_Declare(${K7_ORT_NAME} URL ${K7_ORT_URL} URL_HASH SHA256=${K7_ORT_HASH} DOWNLOAD_EXTRACT_TIMESTAMP ON)
    FetchContent_MakeAvailable(${K7_ORT_NAME})
    set(onnxruntime_SOURCE_DIR ${${K7_ORT_NAME}_SOURCE_DIR})
    # Headers only: the library is LOADED at the first separation, never linked (src/OnnxSession.cpp).
    add_library(k7_onnxruntime INTERFACE)
    if(WIN32)
        target_include_directories(k7_onnxruntime INTERFACE ${onnxruntime_SOURCE_DIR}/build/native/include)
        # DirectML itself, which the ORT package expects to find beside it.
        FetchContent_Declare(directml
            URL      https://api.nuget.org/v3-flatcontainer/microsoft.ai.directml/1.15.4/microsoft.ai.directml.1.15.4.nupkg
            URL_HASH SHA256=4e7cb7ddce8cf837a7a75dc029209b520ca0101470fcdf275c1f49736a3615b9
            DOWNLOAD_EXTRACT_TIMESTAMP ON)
        FetchContent_MakeAvailable(directml)
        set(K7_ORT_RUNTIME ${onnxruntime_SOURCE_DIR}/runtimes/win-x64/native/onnxruntime.dll
                            ${onnxruntime_SOURCE_DIR}/runtimes/win-x64/native/onnxruntime_providers_shared.dll
                            ${directml_SOURCE_DIR}/bin/x64-win/DirectML.dll)
    elseif(IOS)
        string(TOLOWER "${CMAKE_OSX_SYSROOT}" K7_ORT_SYSROOT)
        string(FIND "${K7_ORT_SYSROOT}" "simulator" K7_ORT_SIM)
        if(K7_ORT_SIM EQUAL -1)
            set(K7_ORT_SLICE ios-arm64)
        else()
            set(K7_ORT_SLICE ios-arm64_x86_64-simulator)
        endif()
        target_include_directories(k7_onnxruntime INTERFACE ${onnxruntime_SOURCE_DIR}/Headers)
        target_link_libraries(k7_onnxruntime INTERFACE
            ${onnxruntime_SOURCE_DIR}/onnxruntime.xcframework/${K7_ORT_SLICE}/onnxruntime.framework/onnxruntime
            "-framework Foundation" "-framework UIKit" "-framework Network" "-framework CoreML")
    else()
        # The NuGet package: one archive for macOS and Linux, a native folder per runtime identifier.
        file(GLOB K7_ORT_LIB ${onnxruntime_SOURCE_DIR}/runtimes/${K7_ORT_RID}/native/libonnxruntime.so ${onnxruntime_SOURCE_DIR}/runtimes/${K7_ORT_RID}/native/libonnxruntime.dylib)
        list(GET K7_ORT_LIB 0 K7_ORT_LIB)
        target_include_directories(k7_onnxruntime INTERFACE ${onnxruntime_SOURCE_DIR}/build/native/include)
        set(K7_ORT_RUNTIME ${K7_ORT_LIB})
    endif()
else()
    message(STATUS "ONNX Runtime ships no binary for this platform: building without stem separation")
endif()

# --- the audio decoder: the media library's Rust crate, wherever there is stem separation ----------
# packages/audio-analysis/rust behind rust/include/k7_audio.h, built by cargo into this build tree, so a cut needs no ffmpeg.
if(K7_ORT_URL)
    find_program(K7_CARGO cargo HINTS "$ENV{CARGO_HOME}/bin" "$ENV{HOME}/.cargo/bin" "$ENV{USERPROFILE}/.cargo/bin")
    if(NOT K7_CARGO)
        message(FATAL_ERROR "cargo not found: stem separation decodes with a Rust crate, install it from https://rustup.rs (docs/supervisor.md)")
    endif()
    set(K7_AUDIO_TARGET_DIR ${CMAKE_BINARY_DIR}/cargo)
    set(K7_AUDIO_OUT ${K7_AUDIO_TARGET_DIR}/release)
    set(K7_CARGO_ARGS build --release --locked --manifest-path ${CMAKE_CURRENT_SOURCE_DIR}/rust/Cargo.toml --target-dir ${K7_AUDIO_TARGET_DIR})
    set(K7_CARGO_ENV "")
    if(IOS)
        list(APPEND K7_CARGO_ENV IPHONEOS_DEPLOYMENT_TARGET=${CMAKE_OSX_DEPLOYMENT_TARGET})
        if(K7_ORT_SIM EQUAL -1)
            set(K7_AUDIO_TRIPLE aarch64-apple-ios)
        else()
            set(K7_AUDIO_TRIPLE aarch64-apple-ios-sim)
        endif()
        list(APPEND K7_CARGO_ARGS --target ${K7_AUDIO_TRIPLE})
        set(K7_AUDIO_OUT ${K7_AUDIO_TARGET_DIR}/${K7_AUDIO_TRIPLE}/release)
    elseif(APPLE)
        list(APPEND K7_CARGO_ENV MACOSX_DEPLOYMENT_TARGET=${CMAKE_OSX_DEPLOYMENT_TARGET})
        if(CMAKE_OSX_ARCHITECTURES STREQUAL "arm64")
            list(APPEND K7_CARGO_ARGS --target aarch64-apple-darwin)
            set(K7_AUDIO_OUT ${K7_AUDIO_TARGET_DIR}/aarch64-apple-darwin/release)
        endif()
    endif()
    set(K7_AUDIO_LIB ${K7_AUDIO_OUT}/${CMAKE_STATIC_LIBRARY_PREFIX}k7_audio${CMAKE_STATIC_LIBRARY_SUFFIX})
    set(K7_AUDIO_CRATE ${CMAKE_CURRENT_SOURCE_DIR}/../../packages/audio-analysis/rust)
    file(GLOB_RECURSE K7_AUDIO_SOURCES CONFIGURE_DEPENDS ${CMAKE_CURRENT_SOURCE_DIR}/rust/src/*.rs ${K7_AUDIO_CRATE}/src/*.rs)
    add_custom_command(OUTPUT ${K7_AUDIO_LIB}
        COMMAND ${CMAKE_COMMAND} -E env ${K7_CARGO_ENV} ${K7_CARGO} ${K7_CARGO_ARGS}
        DEPENDS ${K7_AUDIO_SOURCES} ${CMAKE_CURRENT_SOURCE_DIR}/rust/Cargo.toml ${CMAKE_CURRENT_SOURCE_DIR}/rust/Cargo.lock ${K7_AUDIO_CRATE}/Cargo.toml
        COMMENT "cargo: the audio decoder (rust/)"
        USES_TERMINAL
        VERBATIM)
    add_custom_target(k7_audio_build DEPENDS ${K7_AUDIO_LIB})
    add_library(k7_audio STATIC IMPORTED)
    set_target_properties(k7_audio PROPERTIES
        IMPORTED_LOCATION ${K7_AUDIO_LIB}
        INTERFACE_INCLUDE_DIRECTORIES ${CMAKE_CURRENT_SOURCE_DIR}/rust/include)
    # What Rust's standard library links against (`cargo rustc -- --print native-static-libs`).
    if(WIN32)
        set_property(TARGET k7_audio PROPERTY INTERFACE_LINK_LIBRARIES kernel32 advapi32 ntdll userenv ws2_32 dbghelp)
    elseif(APPLE)
        set_property(TARGET k7_audio PROPERTY INTERFACE_LINK_LIBRARIES "-framework CoreFoundation" iconv m)
    else()
        set_property(TARGET k7_audio PROPERTY INTERFACE_LINK_LIBRARIES pthread dl m rt)
    endif()
endif()

# --- the program ----------------------------------------------------------------------------------
# Everything but main.cpp is a library, so the tests link exactly what ships.
add_library(k7_supervisor_core STATIC
    src/Loop.cpp
    src/Crash.cpp
    src/Paths.cpp
    src/Log.cpp
    src/Processes.cpp
    src/PidFile.cpp
    src/Settings.cpp
    src/Slots.cpp
    src/SlotFiles.cpp
    src/Identity.cpp
    src/HostLock.cpp
    src/Url.cpp
    src/HostProtocol.cpp
    src/AudioPolicy.cpp
    src/EngineProcess.cpp
    src/CrashLoop.cpp
    src/WireVersion.cpp
    src/EngineQueries.cpp
    src/ServedRender.cpp
    src/TrustRoots.cpp
    src/Tls.cpp
    src/Http.cpp
    src/Registry.cpp
    src/Addresses.cpp
    src/Performance.cpp
    src/FileTime.cpp
    src/TunnelFrames.cpp
    src/Backpressure.cpp
    src/TunnelFiles.cpp
    src/PathMap.cpp
    src/TunnelAgent.cpp
    src/MachineJobs.cpp
    src/RtcFrames.cpp
    src/Rtc.cpp
    src/SeparationAudio.cpp
    src/SeparationDecode.cpp
    src/OnnxSession.cpp
    src/Separation.cpp
    src/Instruments.cpp
    src/Supervisor.cpp)
# The release stamp the heartbeat and the adoption check use, on its one source alone so a new stamp recompiles one file.
set(K7_BUILD_STAMP "$ENV{STUDIO_RELEASE_STAMP}")
string(REGEX REPLACE "[^A-Za-z0-9._-]" "" K7_BUILD_STAMP "${K7_BUILD_STAMP}")
# A snapshot or a local build names the version it was cut from, the root's, which k7-runtime's package.json copies (docs/versioning.md).
file(READ "${CMAKE_CURRENT_LIST_DIR}/../k7-runtime/package.json" K7_PACKAGE_JSON)
string(JSON K7_VERSION GET "${K7_PACKAGE_JSON}" version)
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${CMAKE_CURRENT_LIST_DIR}/../k7-runtime/package.json")
if(K7_BUILD_STAMP STREQUAL "")
    set(K7_BUILD_STAMP "${K7_VERSION}-dev")
elseif(NOT K7_BUILD_STAMP MATCHES "^v[0-9]")
    set(K7_BUILD_STAMP "${K7_VERSION}-${K7_BUILD_STAMP}")
endif()
set_source_files_properties(src/WireVersion.cpp PROPERTIES COMPILE_DEFINITIONS "K7_BUILD=\"${K7_BUILD_STAMP}\"")
# `src` is searched before the system headers, and Windows ignores case: no header here may share a system header's name.
target_include_directories(k7_supervisor_core PUBLIC src)
target_link_libraries(k7_supervisor_core PUBLIC nlohmann_json::nlohmann_json ixwebsocket datachannel mbedtls mbedx509 mbedcrypto k7_miniz)
if(WIN32)
    target_link_libraries(k7_supervisor_core PUBLIC ws2_32 iphlpapi crypt32 dbghelp)
    target_compile_definitions(k7_supervisor_core PUBLIC NOMINMAX WIN32_LEAN_AND_MEAN)
endif()
if(APPLE)
    target_link_libraries(k7_supervisor_core PUBLIC "-framework Security" "-framework CoreFoundation")
endif()
if(K7_ORT_URL)
    target_link_libraries(k7_supervisor_core PUBLIC k7_onnxruntime ${CMAKE_DL_LIBS})
    target_compile_definitions(k7_supervisor_core PUBLIC K7_WITH_ONNX K7_WITH_DECODER)
    target_link_libraries(k7_supervisor_core PUBLIC k7_audio)
    add_dependencies(k7_supervisor_core k7_audio_build)
    if(WIN32)
        target_link_libraries(k7_supervisor_core PUBLIC dxgi)
    endif()
endif()

# The runtime library beside every executable of the build, the first place the loader looks.
function(k7_beside_runtime target)
    foreach(lib ${K7_ORT_RUNTIME})
        add_custom_command(TARGET ${target} POST_BUILD
            COMMAND ${CMAKE_COMMAND} -E copy_if_different ${lib} $<TARGET_FILE_DIR:${target}>)
    endforeach()
endfunction()

add_executable(k7-supervisor src/main.cpp)
target_link_libraries(k7-supervisor PRIVATE k7_supervisor_core)
k7_beside_runtime(k7-supervisor)

# iOS runs no program of an app's own: the supervisor is a framework there, k7supervisor.framework, which
# the app embeds and calls on a thread of its own; the linker folds every archive above into it (engine/k7-runtime/docs/ios.md).
if(IOS)
    add_library(k7-supervisor-embedded SHARED src/Embedded.cpp)
    target_link_libraries(k7-supervisor-embedded PRIVATE k7_supervisor_core)
    set_target_properties(k7-supervisor-embedded PROPERTIES
        FRAMEWORK TRUE
        OUTPUT_NAME k7supervisor
        MACOSX_FRAMEWORK_IDENTIFIER se.frediver.k7.supervisor
        LIBRARY_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR})
endif()

# --- tests: the engine's tiny harness, one binary, no device and no network -----------------------
enable_testing()
add_executable(k7_supervisor_tests
    tests/main_test.cpp
    tests/Url_test.cpp
    tests/SerialQueue_test.cpp
    tests/Settings_test.cpp
    tests/HostProtocol_test.cpp
    tests/AudioPolicy_test.cpp
    tests/Performance_test.cpp
    tests/MachineJobs_test.cpp
    tests/RtcFrames_test.cpp
    tests/SeparationAudio_test.cpp
    tests/SeparationBudget_test.cpp
    tests/SeparationDecode_test.cpp
    tests/HostLock_test.cpp
    tests/Registry_test.cpp
    tests/Tls_test.cpp
    tests/Instruments_test.cpp
    tests/PathMap_test.cpp
    tests/PidFile_test.cpp
    tests/JobSlots_test.cpp
    tests/Log_test.cpp
    tests/Backpressure_test.cpp
    tests/Http_test.cpp
    tests/CrashLoop_test.cpp
    tests/EnginePorts_test.cpp
    tests/Slots_test.cpp)
target_include_directories(k7_supervisor_tests PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/../k7-runtime/tests)
target_link_libraries(k7_supervisor_tests PRIVATE k7_supervisor_core)
# The frames HostProtocol_test.cpp compares with, as schema/hostProtocol.schema.json describes them.
target_compile_definitions(k7_supervisor_tests PRIVATE
    K7_HOST_PROTOCOL_FIXTURES="${CMAKE_CURRENT_SOURCE_DIR}/schema/fixtures")
k7_beside_runtime(k7_supervisor_tests)
add_test(NAME k7_supervisor_tests COMMAND k7_supervisor_tests)

# The agent alone, for packages/engine-tunnel's suites to run against the real TypeScript hub.
add_executable(tunnel_harness tests/tunnel_harness.cpp)
target_link_libraries(tunnel_harness PRIVATE k7_supervisor_core)
k7_beside_runtime(tunnel_harness)

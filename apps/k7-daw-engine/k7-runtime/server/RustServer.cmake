# RustServer.cmake — the Rust server (apps/k7-daw-backend/packages/k7-runtime-server) as a static library the engine
# targets link: `k7_runtime_server`. cargo runs from the build itself, so every way into this CMake tree
# (build.mjs, the tests' script, Gradle, Xcode's stage.mjs) builds it. Why a static library: that crate's docs/link.md.
# What a reader gets wrong: cargo's folder is the build folder's own `rust/`, named with --target-dir, never CARGO_TARGET_DIR (sccache).

find_program(K7_CARGO cargo HINTS "$ENV{CARGO_HOME}/bin" "$ENV{HOME}/.cargo/bin" "$ENV{USERPROFILE}/.cargo/bin" REQUIRED)

# The triple a phone needs; empty is the host's own, whose archive cargo puts one folder higher.
set(_k7_triple "")
set(_k7_env "K7_UNUSED=1")
if(ANDROID)
    set(_k7_triple aarch64-linux-android)
elseif(IOS)
    set(_k7_env IPHONEOS_DEPLOYMENT_TARGET=${CMAKE_OSX_DEPLOYMENT_TARGET})
    if(CMAKE_OSX_SYSROOT MATCHES "[Ss]imulator")
        set(_k7_triple aarch64-apple-ios-sim)
    else()
        set(_k7_triple aarch64-apple-ios)
    endif()
elseif(APPLE)
    set(_k7_env MACOSX_DEPLOYMENT_TARGET=${CMAKE_OSX_DEPLOYMENT_TARGET})
endif()

set(K7_RUST_DIR "${CMAKE_BINARY_DIR}/rust")
set(_k7_out "${K7_RUST_DIR}")
set(_k7_target_args "")
if(_k7_triple)
    set(_k7_out "${_k7_out}/${_k7_triple}")
    set(_k7_target_args --target ${_k7_triple})
endif()
if(WIN32)
    set(K7_RUST_ARCHIVE "${_k7_out}/release/k7_runtime_server.lib")
else()
    set(K7_RUST_ARCHIVE "${_k7_out}/release/libk7_runtime_server.a")
endif()

add_custom_target(k7_runtime_server_cargo
    COMMAND ${CMAKE_COMMAND} -E env ${_k7_env}
            "${K7_CARGO}" build --release --lib -p k7-runtime-server ${_k7_target_args} --target-dir "${K7_RUST_DIR}"
    WORKING_DIRECTORY "${CMAKE_CURRENT_LIST_DIR}/../../../k7-daw-backend"
    BYPRODUCTS "${K7_RUST_ARCHIVE}"
    USES_TERMINAL VERBATIM)

# The archive by its path, which is what orders the link after cargo; and what Rust's std needs of the platform.
add_library(k7_runtime_server INTERFACE)
target_link_libraries(k7_runtime_server INTERFACE "${K7_RUST_ARCHIVE}")
add_dependencies(k7_runtime_server k7_runtime_server_cargo)
if(WIN32)
    target_link_libraries(k7_runtime_server INTERFACE ws2_32 userenv ntdll bcrypt advapi32 synchronization)
elseif(NOT APPLE)
    target_link_libraries(k7_runtime_server INTERFACE ${CMAKE_DL_LIBS} m)
endif()

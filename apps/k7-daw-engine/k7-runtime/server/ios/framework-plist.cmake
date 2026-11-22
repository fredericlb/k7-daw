# framework-plist.cmake — k7_ios_framework_plist(<target>): an iOS framework's Info.plist, versioned and with a MinimumOSVersion.
# CMake's own template leaves both versions empty and has no MinimumOSVersion, which App Store Connect refuses (docs/ios.md).
# The version is the root's, which k7-runtime's package.json copies (docs/versioning.md).
function(k7_ios_framework_plist target)
    set(json "${CMAKE_CURRENT_FUNCTION_LIST_DIR}/../../package.json")
    file(READ "${json}" text)
    string(JSON version GET "${text}" version)
    set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${json}")
    set_target_properties(${target} PROPERTIES
        MACOSX_FRAMEWORK_INFO_PLIST "${CMAKE_CURRENT_FUNCTION_LIST_DIR}/Framework-Info.plist.in"
        MACOSX_FRAMEWORK_SHORT_VERSION_STRING "${version}"
        MACOSX_FRAMEWORK_BUNDLE_VERSION "${version}")
endfunction()
